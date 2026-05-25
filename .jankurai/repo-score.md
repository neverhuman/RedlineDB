# jankurai Repo Score

- Standard: `jankurai`
- Auditor: `1.5.1`
- Schema: `1.9.0`
- Paper edition: `2026.05-ed8`
- Target stack ID: `rust-ts-vite-react-postgres-bounded-python`
- Target stack: `Rust core + TypeScript/React/Vite + PostgreSQL + generated contracts + exception-only Python AI/data service`
- Repo: `.`
- Run ID: `1779668764`
- Started at: `1779668764`
- Elapsed: `330` ms
- Scope: `changed-fast`
- Changed: `.jankurai/repo-score.json, .jankurai/repo-score.md, crates/cli/src/lib.rs`
- Advisory: `changed-fast scans only changed files plus required control files; run the full audit before merge or release.`
- Raw score: `52`
- Final score: `52`
- Decision: `fail`
- Minimum score: `85`
- Caps applied: `no-one-command-setup-or-validation, no-security-lane-on-high-risk-repo, release-readiness-gap, missing-rust-property-or-integration-tests, no-agent-friendly-exception-pattern, missing-agent-readable-docs, ci-local-parity`

## Hard Rule Caps

| Rule | Max Score | Applied |
| --- | ---: | --- |
| `no-root-agent-instructions` | 75 | no |
| `no-one-command-setup-or-validation` | 70 | yes |
| `no-deterministic-fast-lane` | 65 | no |
| `no-security-lane-on-high-risk-repo` | 60 | yes |
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
| `missing-rust-property-or-integration-tests` | 82 | yes |
| `no-agent-friendly-exception-pattern` | 76 | yes |
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
| `ci-local-parity` | 70 | yes |

## Copy-Code Redundancy

- Status: `skipped` hard=`0` warning=`0` files=`0`
- Policy: min-lines=`10` min-tokens=`100` max-findings=`50` include-tests=`false` strict=`false`
- Duplicate volume: lines=`0` tokens=`0` bytes=`0`

## Dimensions

| Dimension | Weight | Score | Weighted | Evidence |
| --- | ---: | ---: | ---: | --- |
| Ownership and navigation surface | 13 | 44 | 5.72 | root `AGENTS.md` present; owner map covers audited paths |
| Contract and boundary integrity | 13 | 43 | 5.59 | Rust typed boundary helpers found |
| Proof lanes and test routing | 12 | 65 | 7.80 | deterministic fast lane found; test runner present in automation surface |
| Security and supply-chain posture | 12 | 66 | 7.92 | lockfile present; secret or dependency scan tooling found |
| Code shape and semantic surface | 12 | 35 | 4.20 | largest authored code file: crates/cli/src/lib.rs (1130 LOC); code file exceeds 500 LOC |
| Data truth and workflow safety | 8 | 50 | 4.00 |  |
| Observability and repair evidence | 8 | 23 | 1.84 | repair receipts or raw artifact language found |
| Context economy and agent instructions | 7 | 45 | 3.15 | root `AGENTS.md` present; root `AGENTS.md` stays short |
| Jankurai tool adoption and CI replacement | 7 | 80 | 5.60 | control-plane files present; applicable=14 |
| Python containment and polyglot hygiene | 4 | 100 | 4.00 | no Python files in scope |
| Build speed signals | 4 | 60 | 2.40 | build acceleration markers found; targeted test/build commands found |

## Reference Profile Structure

- Applicable cells: `1` canonical=`0` noncanonical=`1` guidance missing=`1`

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
| `ops` | `noncanonical` | `ops/` | `.github, .github/workflows` | `.github/, .github/workflows/, ci/, release/, observability/, security/` | `missing` | `ops` | `security lane / workflow lint` | `migrate the detected ops surface to `ops/` or document an alternate profile with owner, proof lane, expiry, and migration plan` |

## Rendered UX QA

- Web surface: `false`
- Layered UX lane: `true`
- Missing: `none`

## Tool Adoption

- Control plane present: `true`
- Applicable tools: `14`
- Configured: `0`
- CI evidence: `14`
- Artifact verified: `14`
- Replaced count: `14`
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
| `contract-drift` | `contract` | `auto` | `not_applicable` | `handwritten contract drift checks, openapi diff` | `agent/repo-score.json, agent/repo-score.md` |
| `rust-witness` | `rust` | `auto` | `artifact_verified` | `manual witness graphing` | `target/jankurai/rust/witness-graph.json` |
| `vibe-coverage` | `audit` | `auto` | `not_applicable` | `manual vibe-coding coverage spreadsheet` | `target/jankurai/vibe-coverage.json, target/jankurai/vibe-coverage.md` |
| `coverage-evidence` | `proof` | `auto` | `not_applicable` | `manual coverage report review, ad hoc mutation survivor review` | `target/jankurai/coverage/coverage-audit.json, target/jankurai/coverage/coverage-audit.md` |
| `authz-matrix` | `security` | `auto` | `artifact_verified` | `manual authz matrix review` | `agent/repo-score.json, agent/repo-score.md` |
| `input-boundary` | `security` | `auto` | `not_applicable` | `manual unsafe sink review` | `agent/repo-score.json, agent/repo-score.md` |
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
   Reason: `Code shape and semantic surface` scored 35 below the standard floor of 85
   Fix: split large or ambiguous authored code into smaller semantic modules with focused tests
   Rerun: `just fast`
   Fingerprint: `sha256:7e8b4f1c26d4c0f41411477f4305e284de61c535221228eb9874da6a7cf211c1`
   Evidence: largest authored code file: crates/cli/src/lib.rs (1130 LOC), code file exceeds 500 LOC, code file exceeds 1000 LOC, rust bad-behavior advisory signals: 19
2. `high` `proof` `.`
   Check: `HLT-000-SCORE-DIMENSION:proof` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `unmapped`
   Reason: no one-command setup or validation lane was detected
   Fix: add a canonical `setup`, `check`, `test`, or `verify` lane in one root command file
   Rerun: `just fast`
   Fingerprint: `sha256:7010147691f443ae19d3d8603c11ec84958d455885b09e09eca0b9fa91933bde`
   Evidence: no root setup/check/test/verify target surfaced
3. `medium` `context` `.github`
   Rule: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP`
   Check: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `docs/audit-rubric.md#required-shape`
   Reason: reference-profile cell `ops` is detected at a noncanonical path
   Fix: migrate the detected ops surface to `ops/` or document an alternate profile with owner, proof lane, expiry, and migration plan
   Rerun: `just fast`
   Fingerprint: `sha256:12a7cb3de44727e5607afe0a2df603f1f07be2916aa0ede17340426bdc33d1f7`
   Evidence: canonical_path=ops/, detected_paths=.github, .github/workflows, aliases=.github/, .github/workflows/, ci/, release/, observability/, security/, guidance_status=missing, owner=ops, proof_lane=security lane / workflow lint
4. `high` `security` `.github/workflows`
   Rule: `HLT-009-GENERATED-SECURITY`
   Check: `HLT-009-GENERATED-SECURITY:security` `hard` confidence `0.95`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Reason: high-risk repo has no explicit security lane
   Fix: add a dedicated security lane with secret scanning, dependency review, and workflow linting
   Rerun: `just security`
   Fingerprint: `sha256:c249be982d975721833fe396cdfff422f53a2d61819df881968fba63fdd6b9bf`
   Evidence: no security lane markers found
5. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.lib-missing`
   Reason: ops/ci/lib.sh is the shared helper module (artifact assertions, tool pins) every lane sources
   Fix: add ops/ci/lib.sh defining shared helpers and tool version pins
   Rerun: `just fast`
   Fingerprint: `sha256:1991b318cb68e7d158d2872f47f6e4698eb2aca6768d4d912c31d6f943f74454`
   Evidence: detector=ci.local-parity.lib-missing, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
6. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.pre-push-hook-missing`
   Reason: without a mandatory pre-push gate, broken code can be pushed and CI is the first place a failure shows up
   Fix: add ops/git-hooks/pre-push that runs `bash ops/ci/quality-gates.sh` and wire it via `git config core.hooksPath ops/git-hooks`
   Rerun: `just fast`
   Fingerprint: `sha256:6bfbb5bdddc89654f88ac6c8ee166a84a0f6c585adeb94ab8f1e90a0ced148b7`
   Evidence: detector=ci.local-parity.pre-push-hook-missing, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
7. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.toolchain-not-pinned`
   Reason: without a pinned toolchain, local and CI Rust versions can drift silently
   Fix: add rust-toolchain.toml pinning the channel and required components
   Rerun: `just fast`
   Fingerprint: `sha256:74a357e809d6139da803756394fac01291989a8f4b2b140ca8e27a91e29c46ca`
   Evidence: detector=ci.local-parity.toolchain-not-pinned, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
8. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.doctor-missing`
   Reason: without a doctor script, developers cannot confirm their local environment matches CI
   Fix: add scripts/ci-doctor.sh listing every tool the ops/ci scripts depend on
   Rerun: `just fast`
   Fingerprint: `sha256:beef8b01b8447985b9c3fb008eb50af0e8f2e58cf64fd1be36c44d4eeb46d602`
   Evidence: detector=ci.local-parity.doctor-missing, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
9. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.runner-missing`
   Reason: scripts/ci-local.sh is the local entry point that delegates to the same ops/ci scripts the workflows call
   Fix: add scripts/ci-local.sh exposing each CI lane locally
   Rerun: `just fast`
   Fingerprint: `sha256:65427f2c3aa76a8a34f7a609ea37d0405490203f08708acc06c9374be0e492d9`
   Evidence: detector=ci.local-parity.runner-missing, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
10. `high` `ci` `.github/workflows/ci.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.script-missing`
   Reason: missing scripts mean the local runner cannot reproduce the CI step
   Fix: create the referenced ops/ci script with the same commands the workflow used to run
   Rerun: `just fast`
   Fingerprint: `sha256:6fd989baadfed487d1f5858af58297b91cfa03225a7016fe053a02e62e5d90ca`
   Evidence: detector=ci.local-parity.script-missing, path=.github/workflows/ci.yml, line=1, proof_window=None, snippet=name: ci
11. `high` `ci` `.github/workflows/jankurai-tools.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.script-missing`
   Reason: missing scripts mean the local runner cannot reproduce the CI step
   Fix: create the referenced ops/ci script with the same commands the workflow used to run
   Rerun: `just fast`
   Fingerprint: `sha256:6e2f06d88000364d6f14d3c2bba6673a6d677049ea02fbfb850f3811ee546f5d`
   Evidence: detector=ci.local-parity.script-missing, path=.github/workflows/jankurai-tools.yml, line=1, proof_window=None, snippet=name: jankurai-tools
12. `medium` `security` `.github/workflows/jankurai.yml`
   Rule: `HLT-016-SUPPLY-CHAIN-DRIFT`
   Check: `HLT-016-SUPPLY-CHAIN-DRIFT:security` `soft` confidence `0.76`
   Route: TLR `Security, secrets, agency`, lane `security`, owner `ops`
   Docs: `docs/audit-rubric.md#top-level-risk-mapping`
   Reason: `Security and supply-chain posture` scored 66 below the standard floor of 85
   Fix: wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Rerun: `just security`
   Fingerprint: `sha256:cf6eec501e2ed8d1b1b665664eb7b0bd8b854b31207a11a36c3a699e972a0086`
   Evidence: lockfile present, secret or dependency scan tooling found, provenance/SBOM tooling found, workflow linting tooling found
13. `high` `ci` `.github/workflows/jankurai.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.script-missing`
   Reason: missing scripts mean the local runner cannot reproduce the CI step
   Fix: create the referenced ops/ci script with the same commands the workflow used to run
   Rerun: `just fast`
   Fingerprint: `sha256:9d1efc3c6547e854d812122da68fe084f4fd645d7ff6368e15787b5946fcd78d`
   Evidence: detector=ci.local-parity.script-missing, path=.github/workflows/jankurai.yml, line=1, proof_window=None, snippet=name: jankurai
14. `high` `ci` `.github/workflows/release-build.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.script-missing`
   Reason: missing scripts mean the local runner cannot reproduce the CI step
   Fix: create the referenced ops/ci script with the same commands the workflow used to run
   Rerun: `just fast`
   Fingerprint: `sha256:b790943aaff8144a344d3277fc015c48ebe71af808e63a4f1eb6006c456763d4`
   Evidence: detector=ci.local-parity.script-missing, path=.github/workflows/release-build.yml, line=1, proof_window=None, snippet=name: release-build
15. `high` `ci` `.github/workflows/sqlite-parity-report.yml:1`
   Rule: `HLT-042-CI-LOCAL-PARITY`
   Check: `HLT-042-CI-LOCAL-PARITY:ci` `hard` confidence `0.95`
   Route: TLR `Verification`, lane `fast`, owner `ops`
   Docs: `docs/ci-local.md`
   Matched term: `ci.local-parity.script-missing`
   Reason: missing scripts mean the local runner cannot reproduce the CI step
   Fix: create the referenced ops/ci script with the same commands the workflow used to run
   Rerun: `just fast`
   Fingerprint: `sha256:7aaa615018d4c0d9cf01c97f73deed15b1c663ad47923711d07e0fec3068133f`
   Evidence: detector=ci.local-parity.script-missing, path=.github/workflows/sqlite-parity-report.yml, line=1, proof_window=None, snippet=name: redline-testing-report
16. `medium` `context` `AGENTS.md`
   Rule: `HLT-015-CONTEXT-SETUP-GAP`
   Check: `HLT-015-CONTEXT-SETUP-GAP:context` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `docs/agent-native-standard.md`
   Reason: `Context economy and agent instructions` scored 45 below the standard floor of 85
   Fix: keep root guidance short and route durable detail through agent-readable manifests and docs
   Rerun: `just fast`
   Fingerprint: `sha256:53596b312fab41002aaff5de11560e6ff0df9c1bc50139e11140dae0299fd45c`
   Evidence: root `AGENTS.md` present, root `AGENTS.md` stays short, thin IDE/agent adapters are present, missing agent-readable docs: README.md, docs/architecture.md or docs/boundaries.md, docs/testing.md
17. `medium` `proof` `Justfile`
   Rule: `HLT-018-PERF-CONCURRENCY-DRIFT`
   Check: `HLT-018-PERF-CONCURRENCY-DRIFT:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `workspace`
   Docs: `docs/testing.md`
   Reason: `Build speed signals` scored 60 below the standard floor of 85
   Fix: add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Rerun: `just fast`
   Fingerprint: `sha256:938f4ac0d852cab12cdb5c2c7cdac775db884de8ede5176d149a8b50d60d45ae`
   Evidence: build acceleration markers found, targeted test/build commands found, locked dependency graph present, CI cache hint found
18. `medium` `boundary` `agent/boundaries.toml`
   Rule: `HLT-007-HANDWRITTEN-CONTRACT`
   Check: `HLT-007-HANDWRITTEN-CONTRACT:boundary` `soft` confidence `0.76`
   Route: TLR `Contracts/data`, lane `contract`, owner `agent`
   Docs: `docs/audit-rubric.md#known-vibe-coding-insults`
   Reason: `Contract and boundary integrity` scored 43 below the standard floor of 85
   Fix: add generated contracts and boundary checks for public APIs, data access, and cross-runtime seams
   Rerun: `just fast`
   Fingerprint: `sha256:77d2f8c428fc49a7e10198dc4b2aad5b6690c0dfa6dcd49708fa6214375d5a9a`
   Evidence: Rust typed boundary helpers found
19. `medium` `context` `agent/owner-map.json`
   Rule: `HLT-003-OWNERLESS-PATH`
   Check: `HLT-003-OWNERLESS-PATH:context` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#ownership-boundaries`
   Reason: `Ownership and navigation surface` scored 44 below the standard floor of 85
   Fix: tighten owner/test maps and root routing until agents can localize ownership without inference
   Rerun: `just fast`
   Fingerprint: `sha256:c0747472b33e97b5a4286f439aa5e63e16e12094f916f6d6cded0aa78571fd90`
   Evidence: root `AGENTS.md` present, owner map covers audited paths, test map covers audited paths, authored code file exceeds 500 LOC
20. `medium` `proof` `agent/test-map.json`
   Rule: `HLT-004-UNMAPPED-PROOF`
   Check: `HLT-004-UNMAPPED-PROOF:proof` `soft` confidence `0.76`
   Route: TLR `Verification`, lane `fast`, owner `agent`
   Docs: `agent/JANKURAI_STANDARD.md#proof-lanes`
   Reason: `Proof lanes and test routing` scored 65 below the standard floor of 85
   Fix: route each owned path to a deterministic proof command and make the lane executable in CI
   Rerun: `just fast`
   Fingerprint: `sha256:f1b063f8c12a90d80fac1ab40e8463c565d474d78dc43853c566271b71a512bb`
   Evidence: deterministic fast lane found, test runner present in automation surface, GitHub workflow files present, jankurai audit lane found in CI
21. `high` `test` `crates/`
   Rule: `HLT-008-FALSE-GREEN-RISK`
   Check: `HLT-008-FALSE-GREEN-RISK:test` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `fast`, owner `tools`
   Docs: `docs/testing.md`
   Reason: Rust surface lacks required property and/or integration tests
   Fix: add `proptest` or equivalent invariant tests plus `tests/` integration coverage routed through `cargo nextest` or `cargo test`
   Rerun: `just fast`
   Fingerprint: `sha256:8ece7234070a20910736663e65a530625acd16dac7fa57476cfc7c9a74bd745c`
   Evidence: Rust surface detected
22. `high` `exceptions` `crates/domain`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:exceptions` `hard` confidence `0.88`
   Route: TLR `Repair`, lane `observability`, owner `tools`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: no agent-friendly exception/error pattern was detected
   Fix: define a typed exception surface with purpose, reason, common fixes, docs_url, and repair_hint so the next rerun is local
   Rerun: `just score`
   Fingerprint: `sha256:538667a01e35d8e91eae100627364816dd225911862fa2fa1578642af63d4af8`
   Evidence: route repair work to the next agent, opaque failures slow local debugging and reruns, add a typed repair hint; name the common fixes; point at the local docs URL, docs/testing.md
23. `medium` `data` `db/`
   Rule: `HLT-006-DIRECT-DB-WRONG-LAYER`
   Check: `HLT-006-DIRECT-DB-WRONG-LAYER:data` `soft` confidence `0.76`
   Route: TLR `Contracts/data`, lane `db`, owner `tools`
   Docs: `docs/audit-rubric.md#required-shape`
   Reason: `Data truth and workflow safety` scored 50 below the standard floor of 85
   Fix: move durable truth into migrations, constraints, adapters, and application-owned transactions
   Rerun: `just fast`
   Fingerprint: `sha256:6dc277f838aa42b508c136f6ba666d602ecefe226bc4c238b24388640ee21f82`
   Evidence: Data truth and workflow safety scored 50
24. `medium` `docs` `docs/`
   Check: `HLT-000-SCORE-DIMENSION:docs` `soft` confidence `0.76`
   Route: TLR `Context/setup`, lane `audit`, owner `standard`
   Reason: agent-readable documentation is incomplete
   Fix: add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Rerun: `just score`
   Fingerprint: `sha256:7a7bbff17bd45fa833f208a469d73fc717e5fd8687e3d8d20098aa2ce66f2e92`
   Evidence: README.md, docs/architecture.md or docs/boundaries.md, docs/testing.md
25. `high` `release` `docs/release.md`
   Rule: `HLT-025-RELEASE-READINESS-GAP`
   Check: `HLT-025-RELEASE-READINESS-GAP:release` `hard` confidence `0.88`
   Route: TLR `Verification`, lane `release`, owner `standard`
   Docs: `docs/testing.md`
   Matched term: `release structure`
   Reason: launch gates need artifact-backed release evidence
   Fix: add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Rerun: `just check`
   Fingerprint: `sha256:d8ec9f107cf44462f7b5d4065db1938c765752b9912de361a2ae5bdcfc4c0e6c`
   Evidence: release structure missing: changelog, release process doc, rollback guidance
26. `medium` `observability` `docs/testing.md`
   Rule: `HLT-017-OPAQUE-OBSERVABILITY`
   Check: `HLT-017-OPAQUE-OBSERVABILITY:observability` `soft` confidence `0.76`
   Route: TLR `Repair`, lane `observability`, owner `standard`
   Docs: `agent/JANKURAI_STANDARD.md#repair-receipts`
   Reason: `Observability and repair evidence` scored 23 below the standard floor of 85
   Fix: add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Rerun: `just score`
   Fingerprint: `sha256:526e0c991a94e47347e6daa0863998deba126fcca3c278586a4b3bac6a20d7a6`
   Evidence: repair receipts or raw artifact language found, no agent-friendly exception pattern found, free-form logging appears in scope
27. `medium` `release` `docs/testing.md`
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
28. `medium` `context` `ops/`
   Rule: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP`
   Check: `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP:context` `soft` confidence `0.88`
   Route: TLR `Context/setup`, lane `fast`, owner `agent`
   Docs: `docs/audit-rubric.md#required-shape`
   Reason: reference-profile cell `ops` lacks local AGENTS.md guidance
   Fix: add `ops/AGENTS.md` with owns / forbidden / proof lane guidance
   Rerun: `just fast`
   Fingerprint: `sha256:afd6d62dcc0304f7e4872a9edce56c957a74d6c2101cdf6218dce16b4297ba55`
   Evidence: canonical_path=ops/, detected_paths=.github, .github/workflows, guidance_status=missing, owner=ops, proof_lane=security lane / workflow lint

## Policy

- Policy file: `./agent/audit-policy.toml`
- Minimum score: `85`
- Fail on: `critical, high`

## Agent Fix Queue

1. `medium` `HLT-007-HANDWRITTEN-CONTRACT` `agent/boundaries.toml` - add generated contracts and boundary checks for public APIs, data access, and cross-runtime seams
   Route: `Contracts/data`/`contract`
2. `medium` `HLT-006-DIRECT-DB-WRONG-LAYER` `db/` - move durable truth into migrations, constraints, adapters, and application-owned transactions
   Route: `Contracts/data`/`db`
3. `high` `.` - add a canonical `setup`, `check`, `test`, or `verify` lane in one root command file
   Route: `Verification`/`fast`
4. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - add ops/ci/lib.sh defining shared helpers and tool version pins
   Route: `Verification`/`fast`
5. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - add ops/git-hooks/pre-push that runs `bash ops/ci/quality-gates.sh` and wire it via `git config core.hooksPath ops/git-hooks`
   Route: `Verification`/`fast`
6. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - add rust-toolchain.toml pinning the channel and required components
   Route: `Verification`/`fast`
7. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - add scripts/ci-doctor.sh listing every tool the ops/ci scripts depend on
   Route: `Verification`/`fast`
8. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - add scripts/ci-local.sh exposing each CI lane locally
   Route: `Verification`/`fast`
9. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/ci.yml` - create the referenced ops/ci script with the same commands the workflow used to run
   Route: `Verification`/`fast`
10. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/jankurai-tools.yml` - create the referenced ops/ci script with the same commands the workflow used to run
   Route: `Verification`/`fast`
11. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/jankurai.yml` - create the referenced ops/ci script with the same commands the workflow used to run
   Route: `Verification`/`fast`
12. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/release-build.yml` - create the referenced ops/ci script with the same commands the workflow used to run
   Route: `Verification`/`fast`
13. `high` `HLT-042-CI-LOCAL-PARITY` `.github/workflows/sqlite-parity-report.yml` - create the referenced ops/ci script with the same commands the workflow used to run
   Route: `Verification`/`fast`
14. `high` `HLT-008-FALSE-GREEN-RISK` `crates/` - add `proptest` or equivalent invariant tests plus `tests/` integration coverage routed through `cargo nextest` or `cargo test`
   Route: `Verification`/`fast`
15. `high` `HLT-025-RELEASE-READINESS-GAP` `docs/release.md` - add a release control surface with version source, changelog, release process docs, CI or script evidence, integrity/provenance evidence, and rollback guidance
   Route: `Verification`/`release`
16. `medium` `HLT-018-PERF-CONCURRENCY-DRIFT` `Justfile` - add fast deterministic build/test targets, caches, and narrow proof lanes for agent iteration
   Route: `Verification`/`fast`
17. `medium` `HLT-004-UNMAPPED-PROOF` `agent/test-map.json` - route each owned path to a deterministic proof command and make the lane executable in CI
   Route: `Verification`/`fast`
18. `medium` `HLT-026-COST-BUDGET-GAP` `docs/testing.md` - add explicit budgets, quotas, stop conditions, and kill-switch evidence for paid or unbounded operations
   Route: `Verification`/`release`
19. `high` `HLT-017-OPAQUE-OBSERVABILITY` `crates/domain` - define a typed exception surface with purpose, reason, common fixes, docs_url, and repair_hint so the next rerun is local
   Route: `Repair`/`observability`
20. `medium` `HLT-017-OPAQUE-OBSERVABILITY` `docs/testing.md` - add structured errors, telemetry, and repair receipts that tell the next agent where to rerun proof
   Route: `Repair`/`observability`
21. `medium` `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP` `.github` - migrate the detected ops surface to `ops/` or document an alternate profile with owner, proof lane, expiry, and migration plan
   Route: `Context/setup`/`fast`
22. `medium` `HLT-015-CONTEXT-SETUP-GAP` `AGENTS.md` - keep root guidance short and route durable detail through agent-readable manifests and docs
   Route: `Context/setup`/`fast`
23. `medium` `HLT-003-OWNERLESS-PATH` `agent/owner-map.json` - tighten owner/test maps and root routing until agents can localize ownership without inference
   Route: `Context/setup`/`fast`
24. `medium` `docs/` - add concise docs for architecture, boundaries, tests, generated zones, and audit rules; route them from root `AGENTS.md`
   Route: `Context/setup`/`audit`
25. `medium` `HLT-038-REFERENCE-PROFILE-STRUCTURE-GAP` `ops/` - add `ops/AGENTS.md` with owns / forbidden / proof lane guidance
   Route: `Context/setup`/`fast`
26. `high` `HLT-009-GENERATED-SECURITY` `.github/workflows` - add a dedicated security lane with secret scanning, dependency review, and workflow linting
   Route: `Security, secrets, agency`/`security`
27. `medium` `HLT-001-DEAD-MARKER` `.` - split large or ambiguous authored code into smaller semantic modules with focused tests
   Route: `Entropy`/`fast`
28. `medium` `HLT-016-SUPPLY-CHAIN-DRIFT` `.github/workflows/jankurai.yml` - wire secret, dependency, provenance, and workflow scans into an operational CI lane
   Route: `Security, secrets, agency`/`security`
