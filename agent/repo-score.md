# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `0.8.16`
- Schema: `1.7.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1778930136`
- Started at: `1778930136`
- Elapsed: `6048` ms
- Scope: `full`
- Raw score: `82`
- Final score: `70`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `severe-duplication-in-product-code, authz-or-data-isolation-gap`

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

- Status: `review` hard=`1` warning=`70` files=`188`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`190` tokens=`560` bytes=`5403`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set
  - showing the top 50 classes and omitting 21 lower-ranked classes

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitSameName` | `Hard` | `rust` | 31 | 101 | `crates/sql/src/exec/tail_rows.rs:147-178, crates/sql/src/planner/helpers.rs:514-545` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 13 | 49 | `crates/sql/src/exec/expr/scalar/math.rs:99-112, crates/sql/src/parser/helpers.rs:319-332` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 6 | 22 | `crates/kernel/src/format/page.rs:454-460, crates/kernel/src/storage/control.rs:148-154, crates/kernel/src/storage/tx_status_checkpoint.rs:148-154` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:625-626, crates/kernel/src/catalog/stats.rs:391-392, crates/kernel/src/catalog/stats.rs:402-403, crates/kernel/src/catalog/store.rs:608-609, crates/kernel/src/catalog/store.rs:618-619, crates/kernel/src/catalog/store.rs:628-629, crates/kernel/src/catalog/store.rs:638-639, crates/kernel/src/catalog/store.rs:671-672, crates/redlinedb/src/lib.rs:549-550, crates/redlinedb/src/lib.rs:630-631, crates/redlinedb/src/value.rs:106-107, crates/redlinedb/src/value.rs:118-119, crates/redlinedb/src/value.rs:130-131` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 10 | 13 | `crates/redlinedb/src/options.rs:30-40, crates/sql/src/connection/options.rs:42-52` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 4 | `crates/kernel/src/vector/flat.rs:52-55, crates/kernel/src/vector/hnsw/searcher.rs:44-47, crates/sql/src/exec/vec/sort.rs:39-42, crates/sql/src/exec/vec/topk.rs:45-48` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 8 | 31 | `crates/bench/src/report.rs:237-245, crates/bench/src/report.rs:247-255` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 8 | 19 | `crates/sql/src/planner/helpers.rs:469-477, crates/sql/src/planner/helpers.rs:479-487` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 8 | 9 | `crates/redlinedb/src/options.rs:68-76, crates/sql/src/connection/options.rs:80-88` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 7 | 25 | `crates/sql/src/exec/agg.rs:863-870, crates/sql/src/exec/expr/scalar/math.rs:126-133` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 7 | 11 | `crates/redlinedb/src/options.rs:50-57, crates/sql/src/connection/options.rs:62-69` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 6 | 9 | `crates/kernel/src/index/mod.rs:661-667, crates/kernel/src/vector/hnsw/storage.rs:316-322` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:367-368, crates/kernel/src/catalog/stats.rs:371-372, crates/kernel/src/catalog/stats.rs:375-376, crates/kernel/src/catalog/store.rs:587-588, crates/kernel/src/catalog/store.rs:591-592, crates/kernel/src/catalog/store.rs:595-596` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/bench/src/config.rs:395-396, crates/bench/src/config.rs:415-416, crates/kernel/src/vector/diskann/mod.rs:349-350, crates/redlinedb/src/value.rs:40-41, crates/sql/src/exec/expr/scalar/row.rs:345-346, crates/sql/src/exec/expr/scalar/row.rs:355-356` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 0 | `crates/kernel/src/catalog/ddl.rs:142-142, crates/kernel/src/failpoints/mod.rs:41-42, crates/kernel/src/integrity/equivalence.rs:207-207, crates/kernel/src/integrity/page_csum.rs:107-107, crates/sql/src/connection/session.rs:513-513, crates/sql/src/session.rs:207-207` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 13 | `crates/kernel/src/index/locks.rs:191-195, crates/sql/src/session.rs:200-204` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 5 | `crates/kernel/src/format/bytes.rs:22-24, crates/kernel/src/format/bytes.rs:27-29, crates/kernel/src/format/bytes.rs:32-34` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 4 | 8 | `crates/bench/src/certify/scheduler.rs:381-385, crates/bench/src/recover.rs:407-411` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 4 | 8 | `crates/kernel/src/storage/control.rs:156-160, crates/kernel/src/storage/tx_status_checkpoint.rs:156-160` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 11 | `crates/kernel/src/vector/diskann/prune.rs:79-82, crates/kernel/src/vector/diskann/searcher.rs:104-107` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 10 | `crates/sql/src/json/scalar.rs:389-392, crates/sql/src/json/scalar.rs:402-405` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 3 | 7 | `crates/sql/src/exec/index_batch.rs:432-435, crates/sql/src/exec/index_batch.rs:448-451` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 3 | 5 | `crates/sql/src/exec/vec/sort.rs:33-36, crates/sql/src/exec/vec/topk.rs:39-42` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/lib.rs:858-859, crates/redlinedb/src/lib.rs:902-903, crates/redlinedb/src/lib.rs:924-925` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/sql/src/json/path.rs:543-544, crates/sql/src/json/path.rs:551-552, crates/sql/src/json/path.rs:559-560` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 10 | `crates/bench/src/engine/redline.rs:210-212, crates/bench/src/engine/sqlite.rs:269-271` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 7 | `crates/sql/src/planner/helpers.rs:461-463, crates/sql/src/planner/helpers.rs:465-467` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/redlinedb/src/lib.rs:748-749, crates/redlinedb/src/lib.rs:757-758, crates/redlinedb/src/lib.rs:767-768` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 3 | `crates/kernel/src/catalog/store.rs:761-762, crates/kernel/src/catalog/store.rs:769-770, crates/kernel/src/catalog/store.rs:777-778` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 2 | 5 | `crates/bench/src/engine/redline.rs:205-207, crates/bench/src/engine/sqlite.rs:265-267` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 2 | `crates/redlinedb/src/lib.rs:391-392, crates/redlinedb/src/lib.rs:427-428, crates/redlinedb/src/lib.rs:433-434` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 3 | `crates/kernel/src/catalog/key.rs:61-63, crates/kernel/src/index/mod.rs:692-694` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 2 | 1 | `crates/kernel/src/failpoints/mod.rs:65-67, crates/kernel/src/failpoints/mod.rs:109-111` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/lib.rs:516-517, crates/redlinedb/src/lib.rs:654-655` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/lib.rs:218-219, crates/redlinedb/src/lib.rs:233-234` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/redlinedb/src/lib.rs:408-409, crates/redlinedb/src/lib.rs:414-415` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/kernel/src/catalog/stats.rs:458-459, crates/kernel/src/catalog/stats.rs:466-467` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 6 | `crates/sql/src/exec/expr/scalar/row.rs:142-143, crates/sql/src/exec/expr/scalar/row.rs:151-152` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:371-372, crates/kernel/src/catalog/store.rs:591-592` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:367-368, crates/kernel/src/catalog/store.rs:587-588` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:375-376, crates/kernel/src/catalog/store.rs:595-596` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/bench/src/config.rs:537-538, crates/bench/src/config.rs:610-611` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/json/wire.rs:244-245, crates/kernel/src/json/wire.rs:316-317` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:348-349, crates/kernel/src/catalog/store.rs:560-561` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 5 | `crates/kernel/src/catalog/stats.rs:313-314, crates/kernel/src/catalog/store.rs:311-312` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/kernel/src/catalog/stats.rs:387-388, crates/kernel/src/catalog/store.rs:599-600` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/sql/src/exec/expr/scalar/row.rs:171-172, crates/sql/src/exec/expr/scalar/row.rs:214-215` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/lib.rs:592-593, crates/redlinedb/src/lib.rs:676-677` | `same-name semantic unit copied across multiple files` |
| `ExactUnitSameName` | `Warning` | `rust` | 1 | 4 | `crates/redlinedb/src/lib.rs:600-601, crates/redlinedb/src/lib.rs:684-685` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `rust` | 1 | 4 | `crates/domain/src/error.rs:73-74, crates/domain/src/error.rs:91-92` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 100 | 13.00 | root `AGENTS.md` present; `CODEOWNERS` present |
| Contract and boundary integrity | 13 | 98 | 12.74 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 100 | 12.00 | one-command setup/validation lane found; deterministic fast lane found |
| Security and supply-chain posture | 12 | 70 | 8.40 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 45 | 5.40 | largest authored code file: crates/redlinedb/src/lib.rs (941 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 95 | 7.60 | database surface present; structured db boundary manifest present |
| Observability and repair evidence | 8 | 88 | 7.04 | observability libraries or patterns found; ops/observability directory present |
| Context economy and agent instructions | 7 | 100 | 7.00 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 26 | 1.82 | control-plane files present; applicable=16 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
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
   Reason: `Code shape and semantic surface` scored 45 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:4986bb9bf937b16efdb14085461746753d070abb0657a44671a09f25b4845f1f`
   Evidence: largest authored code file: crates/redlinedb/src/lib.rs (941 LOC), code file exceeds 500 LOC, copy-code inexcusable classes found: 1 (exact file or same-name function copy), rust bad-behavior advisory signals: 1753
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
4. `high` `generated` `agent/generated-zones.toml:1`
   Rule: `HLT-002-GENERATED-MUTATION`
   Check: `HLT-002-GENERATED-MUTATION:generated` `hard` confidence `0.95`
   Route: TLR `Contracts/data`, lane `contract`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#generated-zones`
   Reason: generated zone file `crates/ffi/include/redlinedb.h` is missing
   Fix: regenerate `crates/ffi/include/redlinedb.h` using the declared command, or remove the zone entry if the file was deleted intentionally
   Rerun: `just fast`
   Fingerprint: `sha256:df5c62d0365a5a285aac8a6c8d2f5ba9bc5d20e02c2393f56d5bf0f599e87c46`
   Evidence: generated zone integrity violation
5. `high` `audit` `agent/owner-map.json:1`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:audit` `hard` confidence `0.88`
   Route: TLR `Repair`, lane `observability`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: jankurai manifest could not be parsed
   Fix: fix the manifest syntax so audit policy and routing maps are authoritative
   Rerun: `just score`
   Fingerprint: `sha256:4d3a9c009a83e1f29eaffe0eb6c223311fb0d400ca2702b48fc92a31c7f080b8`
   Evidence: key must be a string at line 59 column 1
6. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/index.json` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:63a63918499ec1e3424ac105e60444bb1168ae5e0c338de4663f64f71dd4be84`
   Evidence: benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/index.json
7. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/suites/dick-head-choas-smoke.json` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:64970509ee4a41509d6bb4ba4fc0c2f619d9206cc936d916ea4e9898bda07699`
   Evidence: benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/suites/dick-head-choas-smoke.json
8. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/suites/phase11-oltp-gap.json` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:2c130c5e7ee8796c51315afef4df6e6872360ca7d7617d597d5b9752a8d00c3d`
   Evidence: benchmark-results/version/25262bf7ae8a6d41b46855c55b5dfaad19f85ea8/suites/phase11-oltp-gap.json
9. `high` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: path `python/ai-service/AGENTS.md` has no test-map proof route
   Fix: add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Rerun: `just fast`
   Fingerprint: `sha256:8b6b248cb1ef6e7b7bd5ef77a8b8b95ee31d28fee045d5ee7be75263101b3fc8`
   Evidence: python/ai-service/AGENTS.md
10. `high` `security` `crates/bench/src/config.rs:293`
   Rule: `HLT-022-AUTHZ-ISOLATION-GAP`
   Check: `HLT-022-AUTHZ-ISOLATION-GAP:security` `hard` confidence `0.88`
   Route: TLR `Business truth`, lane `db`, owner `tools`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Matched term: `tenant_id`
   Reason: authz/data isolation requires negative proof evidence
   Fix: add owner/non-owner authorization tests or RLS evidence for the touched data boundary
   Rerun: `just fast`
   Fingerprint: `sha256:a6afd78567ed0a67ae7fc634e599aca5bbb21557df1590c37150845fcec6d7c9`
   Evidence: /// existing `kv_tenant_idx`. Fixture shape mirrors
11. `high` `copy-code` `crates/sql/src/exec/tail_rows.rs:147`
   Rule: `HLT-043-COPY-PASTE-BAD-BEHAVIOR`
   Check: `HLT-043-COPY-PASTE-BAD-BEHAVIOR:copy-code` `hard` confidence `0.95`
   Route: TLR `Maintainability entropy`, lane `copy-code`, owner `tools`
   Docs: `docs/BAD_COPY.md`
   Matched term: `selection_rowid_eq`
   Reason: same-name semantic unit copied across multiple files
   Fix: keep the named unit in one owner and call it from the other sites
   Rerun: `cargo run -p jankurai -- copy-code . --json target/jankurai/copy-code.json --md target/jankurai/copy-code.md`
   Fingerprint: `sha256:8b0119fe9536a56c59a2951b26da7525715e0c061b78ec84fed4edd75e2ac48a`
   Evidence: kind=ExactUnitSameName, language=rust, duplicate_lines=31, duplicate_tokens=101, duplicate_bytes=1039, instances=crates/sql/src/exec/tail_rows.rs:147-178, crates/sql/src/planner/helpers.rs:514-545
12. `medium` `release` `docs/testing.md`
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

1. `high` `HLT-022-AUTHZ-ISOLATION-GAP` `crates/bench/src/config.rs` - add owner/non-owner authorization tests or RLS evidence for the touched data boundary
   Route: `Business truth`/`db`
2. `high` `HLT-002-GENERATED-MUTATION` `agent/generated-zones.toml` - regenerate `crates/ffi/include/redlinedb.h` using the declared command, or remove the zone entry if the file was deleted intentionally
   Route: `Contracts/data`/`contract`
3. `high` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - add the narrowest stable prefix and runnable proof command to `agent/test-map.json`
   Route: `Verification`/`fast`
4. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
5. `medium` `HLT-026-COST-BUDGET-GAP` `docs/testing.md` - add explicit budgets, quotas, stop conditions, and kill-switch evidence for paid or unbounded operations
   Route: `Verification`/`release`
6. `high` `HLT-017-OPAQUE-OBSERVABILITY` `agent/owner-map.json` - fix the manifest syntax so audit policy and routing maps are authoritative
   Route: `Repair`/`observability`
7. `high` `HLT-043-COPY-PASTE-BAD-BEHAVIOR` `crates/sql/src/exec/tail_rows.rs` - keep the named unit in one owner and call it from the other sites
   Route: `Maintainability entropy`/`copy-code`
8. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
9. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
