# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.6.10`
- Schema: `jain-split-ops-v7.0.1-split.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1783499127`
- Started at: `1783499127`
- Elapsed: `1589` ms
- Scope: `full`
- Raw score: `57`
- Final score: `57`
- Decision: `advisory`
- Minimum score: `85`
- Caps applied: `no-one-command-setup-or-validation, no-deterministic-fast-lane, release-readiness-gap, missing-agent-readable-docs`

## Hard Rule Caps

| Rule | Max Score | Applied |
| --- | ---: | --- |
| `no-root-agent-instructions` | 75 | no |
| `no-one-command-setup-or-validation` | 70 | yes |
| `no-deterministic-fast-lane` | 65 | yes |
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
| `release-readiness-gap` | 80 | yes |
| `missing-rust-property-or-integration-tests` | 82 | no |
| `no-agent-friendly-exception-pattern` | 76 | no |
| `missing-agent-readable-docs` | 80 | yes |
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

- Status: `review` hard=`0` warning=`4` files=`4`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`6` tokens=`31` bytes=`206`

- Notes:
  - hard classes are limited to exact active-source file matches and substantial exact same-name units
  - warning classes include same-body different-name units and token/block duplication
  - tests, fixtures, stories, config, Docker, and migrations are omitted unless --include-tests is set

| Kind | Severity | Language | Lines | Tokens | Instances | Reason |
| --- | --- | --- | ---: | ---: | --- | --- |
| `ExactUnitDifferentName` | `Warning` | `python` | 1 | 5 | `ops/split/materialize.py:554-555, ops/split/materialize.py:614-615, ops/split/materialize.py:678-679, ops/split/materialize.py:760-761, ops/split/materialize.py:806-807, ops/split/materialize.py:827-828, ops/split/materialize.py:882-883, ops/split/materialize.py:917-918, ops/split/materialize.py:955-956, ops/split/materialize.py:1617-1618, ops/split/materialize.py:1636-1637, ops/split/materialize.py:1645-1646, ops/split/materialize.py:1687-1688` | `same body appears under different names across files` |
| `ExactUnitDifferentName` | `Warning` | `python` | 2 | 10 | `ops/split/bump-family-version.py:24-28, ops/split/reconcile.py:28-32, ops/split/source_coverage.py:17-21` | `same body appears under different names across files` |
| `ExactUnitSameName` | `Warning` | `python` | 2 | 10 | `ops/split/reconcile.py:28-32, ops/split/source_coverage.py:17-21` | `same-name semantic unit copied across multiple files` |
| `ExactUnitDifferentName` | `Warning` | `python` | 1 | 6 | `ops/split/materialize.py:536-538, ops/split/materialize.py:1002-1004` | `same body appears under different names across files` |

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 88 | 11.44 | root `AGENTS.md` present; owner map present |
| Contract and boundary integrity | 13 | 80 | 10.40 | contract surface found; generated contract artifacts found |
| Proof lanes and test routing | 12 | 40 | 4.80 | test/proof routing map present; web e2e lane present or no web surface |
| Security and supply-chain posture | 12 | 14 | 1.68 | git bad-behavior advisory signals: 1 |
| Code shape and semantic surface | 12 | 90 | 10.80 | no authored adopter product code files in scope |
| Data truth and workflow safety | 8 | 50 | 4.00 |  |
| Observability and repair evidence | 8 | 49 | 3.92 | ops/observability directory present; repair receipts or raw artifact language found |
| Context economy and agent instructions | 7 | 53 | 3.71 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 28 | 1.96 | control-plane files present; applicable=13 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
| Build speed signals | 4 | 0 | 0.00 |  |

## Reference Profile Structure

- Applicable cells: `1` canonical=`1` noncanonical=`0` guidance missing=`1`

| Cell | Status | Canonical | Detected | Aliases | Guidance | Owner | Proof lane | Agent fix |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `web` | `not_applicable` | `apps/web/` | `-` | `frontend/, ui/, packages/web/, packages/ui/` | `not_required` | `apps/web` | `rendered UX / Playwright` | `no action` |
| `api` | `not_applicable` | `apps/api/` | `-` | `api/, server/, backend/` | `not_required` | `apps/api` | `edge handler / contract tests` | `no action` |
| `domain` | `not_applicable` | `crates/domain/` | `-` | `domain/, core/` | `not_required` | `crates/domain` | `unit / property tests` | `no action` |
| `application` | `not_applicable` | `crates/application/` | `-` | `application/, usecases/, use-cases/` | `not_required` | `crates/application` | `use-case / authz tests` | `no action` |
| `adapters` | `not_applicable` | `crates/adapters/` | `-` | `adapters/, infra/, integrations/` | `not_required` | `crates/adapters` | `adapter integration tests` | `no action` |
| `workers` | `not_applicable` | `crates/workers/` | `-` | `workers/, jobs/, scheduler/, queue/` | `not_required` | `crates/workers` | `workflow / replay tests` | `no action` |
| `contracts` | `not_applicable` | `contracts/` | `-` | `openapi/, protobuf/, json-schema/, generated/` | `not_required` | `contracts` | `generation / drift checks` | `no action` |
| `db` | `not_applicable` | `db/` | `-` | `migrations/, constraints/, sql/` | `not_required` | `db` | `migration / constraint tests` | `no action` |
| `python-ai` | `not_applicable` | `python/ai-service/` | `-` | `python/, ai-service/, evals/, embeddings/, model/` | `not_required` | `python/ai-service` | `eval / contract tests` | `no action` |
| `ops` | `canonical` | `ops/` | `ops` | `.github/, .github/workflows/, ci/, release/, observability/, security/` | `missing` | `ops` | `security lane / workflow lint` | `add `ops/AGENTS.md` with owns / forbidden / proof lane guidance` |

## Rendered UX QA

- Web surface: `false`
- Layered UX lane: `true`
- Missing: `none`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `13`
- Configured: `12`
- CI evidence: `0`
- Artifact verified: `0`
- Replaced count: `0`
- Missing CI evidence: `proof-routing, proofbind, proofmark-rust, copy-code, release-bad-behavior, contract-drift, rust-witness, coverage-evidence, authz-matrix, input-boundary, agent-tool-supply, release-readiness, cost-budget`

| Tool | Category | Mode | Status | Replaced | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `audit-ci` | `audit` | `auto` | `not_applicable` | `manual repo scoring, ad hoc score gates` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `proof-routing` | `proof` | `auto` | `configured` | `ad hoc proof lane selection, manual proof receipts` | `.jankurai/repo-score.json, .jankurai/repo-score.md, target/jankurai/repair-queue.jsonl` |
| `proofbind` | `proof` | `auto` | `configured` | `manual changed-surface routing, ad hoc proof obligation lists` | `target/jankurai/proofbind/surface-witness.json, target/jankurai/proofbind/obligations.json` |
| `proofmark-rust` | `proof` | `advisory` | `configured` | `line-only coverage review, manual in-diff mutation review` | `target/jankurai/proofmark/proofmark-receipt.json, target/jankurai/proofmark/proof-receipt.json` |
| `copy-code` | `audit` | `advisory` | `configured` | `ad hoc copy-code review, manual duplication triage` | `target/jankurai/copy-code.json, target/jankurai/copy-code.md` |
| `security` | `security` | `auto` | `not_applicable` | `gitleaks, dependency review, SBOM/provenance` | `target/jankurai/security/evidence.json` |
| `ci-bad-behavior` | `security` | `auto` | `not_applicable` | `mutable workflow refs, secret echo/debug workflow checks, non-blocking security scans` | `target/jankurai/language-bad-behavior.log` |
| `git-bad-behavior` | `audit` | `auto` | `not_applicable` | `destructive git automation, force-push release scripts, hidden stash-based state` | `target/jankurai/language-bad-behavior.log` |
| `release-bad-behavior` | `release` | `auto` | `configured` | `manual release checklist, ad hoc tag and artifact review, manual provenance review` | `target/jankurai/language-bad-behavior.log` |
| `ux-qa` | `ux` | `auto` | `not_applicable` | `playwright, axe-core, visual baselines` | `target/jankurai/ux-qa.json` |
| `db-migration-analyze` | `db` | `auto` | `not_applicable` | `manual migration review` | `target/jankurai/migration-report.json` |
| `contract-drift` | `contract` | `auto` | `configured` | `handwritten contract drift checks, openapi diff` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `rust-witness` | `rust` | `advisory` | `configured` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `missing` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `advisory` | `configured` | `manual authz matrix review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `input-boundary` | `security` | `advisory` | `configured` | `manual unsafe sink review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `agent-tool-supply` | `security` | `auto` | `configured` | `manual MCP/tool trust review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `release-readiness` | `release` | `auto` | `configured` | `manual launch checklist` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |
| `cost-budget` | `release` | `auto` | `configured` | `manual spend review` | `.jankurai/repo-score.json, .jankurai/repo-score.md` |

## Boundary Reclassifications

No audited runtime boundary reclassifications declared.

## Coverage Evidence

- Artifact: `target/jankurai/coverage/coverage-audit.json`
- Status: `missing`
- Sources: total=`0` present=`0`
- Findings: hard=`0` soft=`1`

## Findings

1. `high` `proof` `.`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: no deterministic fast lane was detected
   Fix: add a fast lane that runs the narrowest deterministic proof loop and keep it canonical
   Rerun: `just fast`
   Fingerprint: `sha256:8f5faf682afa829927a322d200bd31f22afd36836db985a72e97a44de7487584`
   Evidence: no fast lane markers found
2. `high` `proof` `.`
   Check: `HLT-000-SCORE-DIMENSION:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `unmapped`
   Reason: no one-command setup or validation lane was detected
   Fix: add a canonical `setup`, `check`, `test`, or `verify` lane in one root command file
   Rerun: `just fast`
   Fingerprint: `sha256:7010147691f443ae19d3d8603c11ec84958d455885b09e09eca0b9fa91933bde`
   Evidence: no root setup/check/test/verify target surfaced
3. `medium` `security` `.github/workflows/jankurai.yml`
   Rule: `HLT-016-SUPPLY-CHAIN-DRIFT`
   Check: `HLT-016-SUPPLY-CHAIN-DRIFT:security` `soft` confidence `0.76`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Reason: `Security and supply-chain posture` scored 14 below the standard floor of 85
   Fix: wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Rerun: `just security`
   Fingerprint: `sha256:b723f587f079b9bcafd978d4da66f53f3dd5977c366abe6683621ebebee67dd9`
   Evidence: git bad-behavior advisory signals: 1, no explicit security lane found, CI does not run the jankurai audit
4. `medium` `proof` `.jankurai/repo-score.json:1743`
   Rule: `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP`
   Check: `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP:proof` `soft` confidence `0.88`
   Route: TLR `Repair`, lane `audit`, owner `audit`
   Docs: `docs/testing.md`
   Matched term: `review evidence`
   Reason: proof and review claims need receipts
   Fix: attach raw CI logs, review receipts, and replayable commands instead of accepting claims or summaries
   Rerun: `just score`
   Fingerprint: `sha256:5b55f43ef5b9cb399e5b35dbd162949c853bb1e3088338f2f09cf9a96a0e5e32`
   Evidence: "# evidence rather than a fabricated \"checked\" log."
5. `medium` `context` `AGENTS.md`
   Rule: `HLT-015-CONTEXT-SETUP-GAP`
   Check: `HLT-015-CONTEXT-SETUP-GAP:context` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `fast`, owner `split`
   Docs: `docs/agent-native-standard.md`
   Reason: `Context economy and agent instructions` scored 53 below the standard floor of 85
   Fix: keep root guidance short and route durable detail through agent-readable manifests and docs
   Rerun: `just fast`
   Fingerprint: `sha256:1f5ed0a8668b1fcdb92bfe0ca907cd220a718aa2504964b369ebe6b480ba01bc`
   Evidence: root `AGENTS.md` present, root `AGENTS.md` stays short, machine-readable routing artifacts present, missing agent-readable docs: README.md, docs/architecture.md or docs/boundaries.md
6. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 0 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:0cf9476a83002d0f15381836511b813d73708c221d9dfc8a5f807059e514534d`
   Evidence: missing one-command setup/validation, missing deterministic fast lane
7. `medium` `boundary` `agent/boundaries.toml`
   Rule: `HLT-007-HANDWRITTEN-CONTRACT`
   Check: `HLT-007-HANDWRITTEN-CONTRACT:boundary` `soft` confidence `0.76`
   Route: TLR `Contracts/data`, lane `contract`, owner `agent`
   Docs: `docs/audit-rubric.md#known-vibe-coding-insults`
   Reason: `Contract and boundary integrity` scored 80 below the standard floor of 85
   Fix: add generated contracts and boundary checks for public APIs, data access, and cross-runtime seams
   Rerun: `just fast`
   Fingerprint: `sha256:903431346d57fc74372397dbc2f24a8cb6be92064e4afaad562725d6db5d272e`
   Evidence: contract surface found, generated contract artifacts found, boundary manifest present, all contract sources have generated zone entries
8. `medium` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: `Proof lanes and test routing` scored 40 below the standard floor of 85
   Fix: route each owned path to a deterministic proof command and make the lane executable in CI
   Rerun: `just fast`
   Fingerprint: `sha256:06d368e66fae00d47a778c1f0c4f495dcb8d12a4effdd09cdd560b16c04fb815`
   Evidence: test/proof routing map present, web e2e lane present or no web surface, rendered UX QA lane present or no web surface, Rust property/integration tests present or no Rust surface
9. `medium` `data` `db/`
   Rule: `HLT-006-DIRECT-DB-WRONG-LAYER`
   Check: `HLT-006-DIRECT-DB-WRONG-LAYER:data` `soft` confidence `0.76`
   Route: TLR `Contracts/data`, lane `db`, owner `tools`
   Docs: `docs/audit-rubric.md#required-shape`
   Reason: `Data truth and workflow safety` scored 50 below the standard floor of 85
   Fix: move durable truth into migrations, constraints, adapters, and application-owned transactions
   Rerun: `just fast`
   Fingerprint: `sha256:6dc277f838aa42b508c136f6ba666d602ecefe226bc4c238b24388640ee21f82`
   Evidence: Data truth and workflow safety scored 50
10. `medium` `docs` `docs/`
   Check: `HLT-000-SCORE-DIMENSION:docs` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `audit`, owner `docs`
   Reason: agent-readable documentation is incomplete
   Fix: add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Rerun: `just score`
   Fingerprint: `sha256:fb48933eb532f63be52604fcb6cffc8a5bf25cd4fb4c1a66c86445ba4fa1017b`
   Evidence: README.md, docs/architecture.md or docs/boundaries.md
11. `high` `release` `docs/release.md`
   Rule: `HLT-025-RELEASE-READINESS-GAP`
   Check: `HLT-025-RELEASE-READINESS-GAP:release` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `release`, owner `docs`
   Docs: `docs/testing.md`
   Matched term: `release structure`
   Reason: launch gates need artifact-backed release evidence
   Fix: add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Rerun: `just check`
   Fingerprint: `sha256:c7eefce130f9057e693ec4f1e52a32ae746bb45e440d5f623037f50ad020472e`
   Evidence: release structure missing: changelog, release process doc
12. `medium` `observability` `docs/testing.md`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:observability` `soft` confidence `0.76`
   Route: TLR `Repair`, lane `observability`, owner `docs`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: `Observability and repair evidence` scored 49 below the standard floor of 85
   Fix: add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Rerun: `just score`
   Fingerprint: `sha256:277f6c3e13b5b169dafcbc2723092aae7fd5a6881d4d80c40f399f3d1cd8eed1`
   Evidence: ops/observability directory present, repair receipts or raw artifact language found, repair receipt guidance is documented, no agent-friendly exception pattern found
13. `medium` `context` `ops/`
   Rule: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP`
   Check: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `ops`
   Docs: `docs/audit-rubric.md#required-shape`
   Reason: reference-profile cell `ops` lacks local AGENTS.md guidance
   Fix: add `ops/AGENTS.md` with owns / forbidden / proof lane guidance
   Rerun: `just fast`
   Fingerprint: `sha256:197b917ee375c1b861069568a8977bd8fb2f0322911c4599fd66b26013bf7cd6`
   Evidence: canonical_path=ops/, detected_paths=ops, guidance_status=missing, owner=ops, proof_lane=security lane / workflow lint
14. `medium` `test` `agent/coverage-sources.toml`
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
2. `medium` `HLT-006-DIRECT-DB-WRONG-LAYER` `db/` - move durable truth into migrations, constraints, adapters, and application-owned transactions
   Route: `Contracts/data`/`db`
3. `high` `HLT-004-UNMAPPED-PROOF` `.` - add a fast lane that runs the narrowest deterministic proof loop and keep it canonical
   Route: `Verification`/`fast`
4. `high` `.` - add a canonical `setup`, `check`, `test`, or `verify` lane in one root command file
   Route: `Verification`/`fast`
5. `high` `HLT-025-RELEASE-READINESS-GAP` `docs/release.md` - add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Route: `Verification`/`release`
6. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
7. `medium` `HLT-008-FALSE-GREEN-RISK` `agent/coverage-sources.toml` - run `cargo run -p jankurai -- coverage audit . --config agent/coverage-sources.toml --json target/jankurai/coverage/coverage-audit.json --md target/jankurai/coverage/coverage-audit.md`
   Route: `Verification`/`coverage-audit`
8. `medium` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - route each owned path to a deterministic proof command and make the lane executable in CI
   Route: `Verification`/`fast`
9. `medium` `HLT-027-HUMAN-REVIEW-EVIDENCE-GAP` `.jankurai/repo-score.json` - attach raw CI logs, review receipts, and replayable commands instead of accepting claims or summaries
   Route: `Repair`/`audit`
10. `medium` `HLT-017-OPAQUE-OBSERVABILITY` `docs/testing.md` - add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Route: `Repair`/`observability`
11. `medium` `HLT-015-CONTEXT-SETUP-GAP` `AGENTS.md` - keep root guidance short and route durable detail through agent-readable manifests and docs
   Route: `Context/setup`/`fast`
12. `medium` `docs/` - add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Route: `Context/setup`/`audit`
13. `medium` `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP` `ops/` - add `ops/AGENTS.md` with owns / forbidden / proof lane guidance
   Route: `Context/setup`/`fast`
14. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
