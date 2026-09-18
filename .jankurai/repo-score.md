# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.6.11`
- Schema: `1.9.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1789744261`
- Started at: `1789744261`
- Elapsed: `22383` ms
- Scope: `full`
- Raw score: `85`
- Final score: `85`
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

- Status: `review` hard=`0` warning=`165` files=`455`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`308` tokens=`955` bytes=`8835`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 115 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/json_tv.rs:228-233, crates/sql/src/exec/json_tv.rs:283-288, crates/sql/src/exec/json_tv.rs:399-404, crates/sql/src/exec/json_tv.rs:433-438, crates/sql/src/exec/json_tv.rs:467-472, crates/sql/src/exec/json_tv.rs:504-509, crates/sql/src/exec/json_tv.rs:541-546, crates/sql/src/exec/json_tv.rs:578-583, crates/sql/src/exec/json_tv.rs:616-621, crates/sql/src/exec/pragma_tv.rs:220-225, crates/sql/src/exec/pragma_tv.rs:243-248, crates/sql/src/exec/pragma_tv.rs:282-287, crates/sql/src/exec/pragma_tv.rs:370-375, crates/sql/src/exec/table_valued.rs:77-82` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 42 | `crates/sql/src/exec/cross_db.rs:203-216, crates/sql/src/exec/cte.rs:181-194, crates/sql/src/exec/view.rs:180-193` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/score_policy.rs:180-181, crates/bench/src/score_policy.rs:188-189, crates/kernel/src/catalog/record.rs:152-153, crates/kernel/src/catalog/stats/wire.rs:162-163, crates/kernel/src/catalog/stats/wire.rs:173-174, crates/kernel/src/catalog/store.rs:1029-1030, crates/kernel/src/catalog/store.rs:1039-1040, crates/kernel/src/catalog/store.rs:1049-1050, crates/kernel/src/catalog/store.rs:1059-1060, crates/kernel/src/catalog/store.rs:1069-1070, crates/kernel/src/catalog/store.rs:1092-1093, crates/redlinedb-sqlx/src/bridge/runtime.rs:390-391, crates/redlinedb-sqlx/src/bridge/runtime.rs:487-488, crates/redlinedb/src/value_conv.rs:261-262, crates/sql/src/exec/expr/coerce/binary.rs:396-397, crates/sql/src/exec/expr/coerce/binary.rs:403-404, crates/sql/src/exec/expr/coerce/binary.rs:512-513, crates/sql/src/exec/expr/json_dispatch.rs:765-766, crates/sql/src/exec/index_access.rs:1094-1095, crates/sql/src/exec/json_tv.rs:241-242, crates/sql/src/json/scalar.rs:134-135, crates/sql/src/json/scalar.rs:151-152, crates/sql/src/json/scalar.rs:181-182, crates/sql/src/json/scalar.rs:643-644` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 7 | 16 | `crates/cli/src/render.rs:472-479, crates/cli/src/render.rs:591-598, crates/cli/src/render.rs:652-659, crates/cli/src/render.rs:726-733` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 9 | `crates/sql/src/exec/pragma_tv.rs:76-81, crates/sql/src/exec/pragma_tv.rs:108-113, crates/sql/src/exec/pragma_tv.rs:134-139, crates/sql/src/exec/pragma_tv.rs:429-434, crates/sql/src/exec/pragma_tv.rs:463-468` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 16 | 37 | `crates/sql/src/exec/agg/select.rs:85-101, crates/sql/src/planner/access/projection.rs:153-169` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/cli/src/dot/control.rs:112-114, crates/cli/src/dot/control.rs:116-118, crates/cli/src/dot/control.rs:120-122, crates/cli/src/dot/control.rs:124-126, crates/cli/src/dot/control.rs:137-139, crates/cli/src/dot/control.rs:291-293, crates/cli/src/dot/control.rs:374-376` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 11 | 57 | `crates/sql/src/datetime/format.rs:75-86, crates/sql/src/datetime/modifiers.rs:193-204` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:413-414, crates/bench/src/config.rs:434-435, crates/redlinedb/src/value.rs:43-44, crates/redlinedb/src/value.rs:57-58, crates/redlinedb/src/value.rs:64-65, crates/redlinedb/src/value.rs:71-72, crates/redlinedb/src/value.rs:78-79, crates/sql/src/exec/expr/scalar/row/model.rs:67-68, crates/sql/src/exec/expr/scalar/row/model.rs:81-82, crates/sql/src/exec/expr/scalar/row/model.rs:92-93, crates/sql/src/statement.rs:364-365, crates/sql/src/statement.rs:661-662` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 10 | 24 | `crates/sql/src/exec/expr/predicate.rs:336-346, crates/sql/src/parser/rewrite/pg_ddl.rs:278-288` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 7 | 15 | `crates/sql/src/parser/select.rs:1625-1632, crates/sql/src/parser/select.rs:1696-1703` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 7 | 12 | `crates/kernel/src/engine/page_heap/policy.rs:78-85, crates/kernel/src/engine/page_heap/policy.rs:108-115` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:313-313, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:214-214, crates/kernel/src/integrity/page_csum.rs:107-107, crates/redlinedb-sqlx/src/bridge/options.rs:253-254, crates/redlinedb-sqlx/src/bridge/runtime.rs:127-128, crates/sql/src/connection/session.rs:1240-1240, crates/sql/src/exec/merge.rs:278-278` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/bridge/options.rs:223-224, crates/redlinedb-sqlx/src/bridge/runtime.rs:52-53, crates/redlinedb-sqlx/src/bridge/runtime.rs:57-58, crates/redlinedb-sqlx/src/bridge/runtime.rs:81-82, crates/redlinedb-sqlx/src/bridge/runtime.rs:96-97, crates/redlinedb-sqlx/src/bridge/runtime.rs:111-112, crates/redlinedb-sqlx/src/bridge/runtime.rs:201-202` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 21 | `crates/cli/src/shellzero.rs:217-223, crates/redlinedb-lite/src/shellzero.rs:216-222` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 20 | `crates/sql/src/exec/agg/select.rs:9-15, crates/sql/src/planner/access/projection.rs:94-100` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/json/path_bytecode.rs:138-140, crates/sql/src/json/jsonb.rs:1062-1064, crates/sql/src/parser/rewrite/pg_ddl.rs:320-322, crates/sql/src/parser/rewrite/pg_ddl.rs:1325-1327` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 12 | `crates/sql/src/exec/policy.rs:32-38, crates/sql/src/exec/policy.rs:57-63` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111, crates/kernel/src/storage/numa.rs:59-61, crates/kernel/src/storage/numa.rs:64-66` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/catalog/store.rs:557-558, crates/sql/src/parser/rewrite/sqlite_shape.rs:1178-1179, crates/sql/src/parser/rewrite/sqlite_shape.rs:1283-1284, crates/sql/src/parser/rewrite/sqlite_shape.rs:1382-1383, crates/sql/src/parser/split.rs:33-34, crates/sql/src/parser/split.rs:171-172` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 15 | `crates/sql/src/exec/mod.rs:1546-1551, crates/sql/src/exec/mod.rs:1561-1566` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 5 | 13 | `crates/sql/src/exec/agg/select.rs:103-108, crates/sql/src/planner/access/projection.rs:171-176` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 12 | `crates/cli/src/render.rs:979-984, crates/cli/src/render.rs:1003-1008` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 5 | 10 | `crates/cli/src/render.rs:459-464, crates/cli/src/render.rs:549-554` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:200-204, crates/sql/src/session.rs:459-463` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 12 | `crates/sql/src/exec/expr/json_dispatch.rs:62-66, crates/sql/src/exec/expr/json_dispatch.rs:822-826` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:44-46, crates/kernel/src/format/bytes.rs:49-51, crates/kernel/src/format/bytes.rs:54-56` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 9 | `crates/sql/src/exec/expr/scalar/row/lookup.rs:68-72, crates/sql/src/exec/expr/scalar/row/lookup.rs:137-141` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/sql/src/exec/expr/program.rs:970-971, crates/sql/src/exec/expr/program.rs:1027-1028, crates/sql/src/exec/expr/program.rs:1042-1043, crates/sql/src/exec/expr/program.rs:1096-1097, crates/sql/src/json/scalar.rs:539-540` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/format/page.rs:102-104, crates/kernel/src/storage/control.rs:156-158, crates/kernel/src/storage/tx_status_checkpoint.rs:156-158` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 5 | `crates/redlinedb/src/connection.rs:172-176, crates/redlinedb/src/connection.rs:187-191` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 1 | `crates/bench/src/sqlite_parity/engine.rs:172-173, crates/redlinedb-sqlx/src/driver.rs:216-217, crates/redlinedb/src/iter.rs:93-94, crates/sql/src/exec/expr/program.rs:372-373, crates/sql/src/exec/morsel/hash_agg.rs:301-302` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 16 | `crates/sql/src/exec/expr/scalar/value.rs:556-559, crates/sql/src/exec/expr/scalar/value.rs:710-713` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/statement.rs:962-963, crates/sql/src/statement.rs:970-971, crates/sql/src/statement.rs:978-979, crates/sql/src/statement.rs:985-986` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/vector/diskann/sectors.rs:419-420, crates/redlinedb/src/value_conv.rs:336-337, crates/redlinedb/src/value_conv.rs:357-358, crates/redlinedb/src/value_conv.rs:378-379` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 10 | `crates/sql/src/exec/index_batch.rs:530-533, crates/sql/src/exec/index_batch.rs:538-541` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/sql/src/rql.rs:1334-1335, crates/sql/src/rql.rs:1419-1420, crates/sql/src/rql.rs:1558-1559, crates/sql/src/rql.rs:1582-1583` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 7 | `crates/sql/src/exec/expr/scalar/value.rs:321-324, crates/sql/src/exec/expr/scalar/value.rs:369-372` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 6 | `crates/sql/src/parser.rs:295-298, crates/sql/src/parser.rs:316-319` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 5 | `crates/ffi/src/sqlite3_api/hooks.rs:72-75, crates/ffi/src/sqlite3_api/hooks_fire.rs:37-40` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:57-60, crates/kernel/src/vector/hnsw/searcher.rs:47-50` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 2 | `crates/kernel/src/engine/tx/status.rs:306-309, crates/sql/src/exec/hot_row.rs:576-579` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/connection/database.rs:622-623, crates/sql/src/connection/database.rs:636-637, crates/sql/src/connection/database.rs:670-671` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 7 | `crates/sql/src/exec/expr/json_dispatch.rs:960-961, crates/sql/src/exec/expr/json_dispatch.rs:972-973, crates/sql/src/exec/expr/json_dispatch.rs:985-986` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb-sqlx/src/driver.rs:399-400, crates/redlinedb-sqlx/src/driver.rs:414-415, crates/redlinedb-sqlx/src/driver.rs:429-430` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/sql/src/json/path.rs:543-544, crates/sql/src/json/path.rs:551-552, crates/sql/src/json/path.rs:559-560` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/cli/src/render.rs:410-411, crates/cli/src/render.rs:430-431, crates/cli/src/render.rs:444-445` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/ops.rs:1486-1487, crates/sql/src/parser/rewrite/sqlite_shape.rs:1002-1003, crates/sql/src/parser/split.rs:164-165` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/sql/src/json/scalar.rs:279-280, crates/sql/src/json/scalar.rs:557-558, crates/sql/src/json/scalar.rs:587-588` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/cli/src/render.rs:641-642, crates/cli/src/render.rs:766-767, crates/cli/src/render.rs:777-778` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 98 | 12.74 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 100 | 12.00 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 70 | 8.40 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 45 | 5.40 | largest authored code file: crates/sql/src/parser/pragma.rs (1951 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 100 | 8.00 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 98 | 7.84 | observability libraries or patterns found; diagnostic shaping hints found |
| Context economy and agent instructions | 7 | 100 | 7.00 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 53 | 3.71 | control-plane files present; applicable=16 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
| Build speed signals | 4 | 70 | 2.80 | build acceleration markers found; targeted test/build commands found |

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
- Configured: `0`
- CI evidence: `10`
- Artifact verified: `8`
- Replaced count: `10`
- Missing CI evidence: `proof-routing, proofbind, proofmark-rust, copy-code, ci-bad-behavior, git-bad-behavior, release-bad-behavior, rust-witness`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `artifact_verified` | `manual repo scoring, ad hoc score gates` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `ci_evidence` | `ad hoc proof lane selection, manual proof receipts` | `.jankurai/repo-score.json, .jankurai/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `missing` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `auto` | `missing` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `auto` | `missing` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `artifact_verified` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `missing` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `missing` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `missing` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `artifact_verified` | `handwritten contract drift checks, openapi diff` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `ci_evidence` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `artifact_verified` | `manual authz matrix review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `input-boundary` | `security` | `auto` | `artifact_verified` | `manual unsafe sink review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `artifact_verified` | `manual MCP/tool trust review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `release-readiness` | `release` | `auto` | `artifact_verified` | `manual launch checklist` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `cost-budget` | `release` | `auto` | `artifact_verified` | `manual spend review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |

## Boundary manifest (ingested)

- Path: `agent/boundaries.toml`
- Stack: `rust-ts-vite-react-postgres-bounded-python` · version: `0.4.0`
- Queue path counts — adapter: `2`, event_contract: `1`, generated_type: `1`, client_marker: `7`, streaming_exception: `1`
- Content fingerprint: `sha256:39f4f2fc80401bd10b3db201e1a8c065a5ba7a77567cf9bd9f3b6069eb40e8be`

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
   Fingerprint: `sha256:30b9addb1c90686f81395eea308052f5ce7ef8c5cd91e200059881474d643438`
   Evidence: largest authored code file: crates/sql/src/parser/pragma.rs (1951 LOC), code file exceeds 500 LOC, code file exceeds 1000 LOC, most code files stay under 300 LOC
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
   Reason: `Build speed signals` scored 70 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:a256a7390d4b91a5b0a95d6f092e524c8f4080f27fe2b62e28cf0801343d0fef`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
4. `medium` `copy-code` `crates/bench/src/fuzz/normalize.rs:19`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `Cell` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `Cell` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:06abf3e40a8c91cd5bd3a90c2801aa5769be3e9a2c1e83dba0430881ddec8173`
   Evidence: enum `Cell` is defined with diverging shapes in 2 modules (crates/bench/src/fuzz/normalize.rs:19, crates/cli/src/render.rs:10)
5. `medium` `copy-code` `crates/kernel/src/error.rs:21`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `Error` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `Error` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:0c9d695968a4c3656809617dc18f78a9c390dfb8bf704adb486db7cd6f8a193f`
   Evidence: enum `Error` is defined with diverging shapes in 2 modules (crates/kernel/src/error.rs:21, crates/sql/src/error.rs:5)
6. `medium` `copy-code` `crates/kernel/src/json/path_bytecode.rs:27`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `Op` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `Op` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:b3bb182f6967fe53e8a9863ef4ed7ac2446a6997b89e3e61cae7832373f73b23`
   Evidence: enum `Op` is defined with diverging shapes in 2 modules (crates/kernel/src/json/path_bytecode.rs:27, crates/sql/src/exec/expr/program.rs:305)
7. `medium` `copy-code` `crates/redlinedb/src/iter.rs:13`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `Step` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `Step` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:125bde9e2b4288bd664bd5f89fd764927858313928a383ba83dc2f1214597a2e`
   Evidence: enum `Step` is defined with diverging shapes in 2 modules (crates/redlinedb/src/iter.rs:13, crates/sql/src/statement.rs:745)
8. `medium` `copy-code` `crates/sql/src/exec/morsel/hash_agg.rs:43`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `AggKind` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `AggKind` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:1792bb4ccf76315d0f67831b6b9614a39d3305eda161b9a07cf4b4a21f986665`
   Evidence: enum `AggKind` is defined with diverging shapes in 2 modules (crates/sql/src/exec/morsel/hash_agg.rs:43, crates/sql/src/exec/vec/hash_agg.rs:23)
9. `medium` `copy-code` `crates/sql/src/planner.rs:113`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `JoinKind` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `JoinKind` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:727b18f3f2599d194006380d54645ddf5dd2248425597bed1813530843f81b7e`
   Evidence: enum `JoinKind` is defined with diverging shapes in 2 modules (crates/sql/src/planner.rs:113, crates/sql/src/statement.rs:491)
10. `medium` `copy-code` `crates/sql/src/planner.rs:121`
   Rule: `HLT-046-UNNECESSARY-VARIETY`
   Check: `HLT-046-UNNECESSARY-VARIETY:copy-code` `soft` confidence `0.88`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#jankurai-pillar-variety-and-canonical-shape`
   Matched term: `unnecessary-variety`
   Reason: enum `AccessPath` has 2 divergent definitions across modules where one consistent definition is expected
   Fix: define `AccessPath` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:1d2df57702312ed0db60cbea557e9e0b0fed4b41a92170f55d8adcf2919e24ae`
   Evidence: enum `AccessPath` is defined with diverging shapes in 2 modules (crates/sql/src/planner.rs:121, crates/sql/src/planner/access_path.rs:104)

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
2. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
3. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
4. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/bench/src/fuzz/normalize.rs` - define `Cell` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
5. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/kernel/src/error.rs` - define `Error` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
6. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/kernel/src/json/path_bytecode.rs` - define `Op` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
7. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/redlinedb/src/iter.rs` - define `Step` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
8. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/sql/src/exec/morsel/hash_agg.rs` - define `AggKind` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
9. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/sql/src/planner.rs` - define `JoinKind` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
10. `medium` `HLT-046-UNNECESSARY-VARIETY` `crates/sql/src/planner.rs` - define `AccessPath` once in a shared module and import it everywhere, or reconcile the diverging definitions so one canonical shape is used; redundant variety lets the copies drift apart
   Route: `Maintainability entropy`/`copy-code`
