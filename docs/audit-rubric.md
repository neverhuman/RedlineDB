# Audit Rubric — RedlineDB

This repo follows the jankurai standard. The audit at `.jankurai/repo-score.md`
scores 11 dimensions; this doc maps each dimension to where the proof lives
in this codebase and which proof lane an agent should rerun to verify a
repair. Pair this with `.jankurai/owner-map.json` (who owns the file) and
`.jankurai/proof-lanes.toml` (how to rerun the proof).

## Dimensions

### 1. Code shape
- Evidence: `./scripts/check_file_sizes.sh`, file LOC budgets in
  `.jankurai/file-size-policy.toml`, debt entries in `.jankurai/debt-map.json`.
- Proof lane: `just fast` (runs `check_file_sizes.sh` plus tests).

### 2. Future-hostile language
- Evidence: source greps for the detector terms enumerated in
  `docs/language-bad-behavior.md`.
- Proof lane: `just fast`, then `jankurai audit . --mode advisory`.

### 3. Repo rot
- Evidence: bench TOMLs under `crates/bench/bench/`, module headers
  in `crates/{ffi,redlinedb}/src/backup.rs`, exception declarations in
  `.jankurai/repo-rot-exceptions.toml` (when present) and
  `.jankurai/generated-zones.toml`.
- Proof lane: `just score`, `just fast`.

### 4. Rust bad behavior (`unsafe`)
- Evidence: every `unsafe` block carries a `// SAFETY:` comment;
  ledgered sites live in `.jankurai/unsafe-ledger.toml`.
- Proof lane: `just fast` plus the FFI-specific
  `crates/ffi/tests/safety_invariants.rs` once Section D lands.

### 5. Non-optimal product language
- Evidence: stack profile declares Rust as the product-truth language;
  generated/ABI surfaces are listed in `.jankurai/generated-zones.toml`
  (`crates/ffi/include/redlinedb.h` is the canonical C-ABI carve-out).
- Proof lane: `just fast`.

### 6. Python containment
- Evidence: Python is bench-and-ops-only; product truth in Rust. Any
  remaining Python lives under `scripts/` or `python/` and is declared
  in `.jankurai/owner-map.json` with an explicit non-product owner.
- Proof lane: `just score`.

### 7. Observability and structured errors
- Evidence: typed exception surface at
  `crates/domain/src/error.rs::DomainError`; kernel escalation path at
  `crates/kernel/src/error.rs::Error::into_domain`; repair-receipt
  template at `.jankurai/proof-receipt-template.md`.
- Proof lane: `rtk cargo test -p redlinedb-domain --quiet --locked` plus
  `rtk cargo test -p redlinedb-kernel --quiet --locked`.

### 8. Agent-readable docs
- Evidence: this file, `docs/language-bad-behavior.md`, `docs/testing.md`,
  and the root `AGENTS.md` router.
- Proof lane: `just score`.

### 9. Authz and data isolation
- Evidence: tenant isolation tests under `crates/bench/tests/` (added in
  Section E), policy declarations in `.jankurai/security-policy.toml`.
- Proof lane: `just fast`, `just security`.

### 10. Input boundary
- Evidence: FFI boundary tests in `crates/ffi/tests/` (Section E),
  parser fuzz targets, JSON path validation in `crates/kernel/src/json/`.
- Proof lane: `just fast`, `just security`.

### 11. Release readiness
- Evidence: `docs/release.md` (Section H), CI workflows in
  `.github/workflows/`, cost budgets in `.jankurai/cost-budget.toml`
  (Section H).
- Proof lane: `just check`, `just security`.

## Top-Level Risk Mapping

Per the jankurai Reference Profile, every TLR cell names its lane and
owner so an agent can route from "this failed" to "who fixes it" in one
hop.

| TLR cell      | Where it lives                                        | Lane                          | Owner                         |
|---------------|-------------------------------------------------------|-------------------------------|-------------------------------|
| web           | (none — RedlineDB ships no web frontend)              | n/a                           | n/a                           |
| api           | `crates/server/`                                      | `just fast`                   | `framed-server`               |
| domain        | `crates/domain/`, `crates/kernel/src/txn/`            | `just fast`                   | `storage-and-catalog`         |
| application   | `crates/sql/`, `crates/redlinedb/`                    | `just fast`                   | `sql-parser-planner-executor` |
| adapters      | `crates/ffi/`, `crates/cli/`                          | `just fast`                   | `c-abi` / `cli-shell`         |
| workers       | `crates/bench/`                                       | `phase9-smoke`, `just fast`   | `bench-harness`               |
| contracts     | `crates/ffi/include/redlinedb.h`, `crates/bench/compat/` | `phase9-compat-full`       | `c-abi`                       |
| db            | `crates/kernel/src/{storage,wal,heap,index}/`         | `phase9-recovery-matrix`      | `storage-and-catalog`         |
| python-ai     | `scripts/`, `python/` (when present)                  | `just score`                  | `agent`                       |
| ops           | `.github/workflows/`, `justfile`, `.jankurai/`            | `just check`, `just security` | `ops` / `agent`               |

## Future-Hostile Language Rule

The detector terms (`placeholder`, `temp`, `legacy`, `compat`, `fallback`,
`todo`, `stub`, `old`, `unused`, `stale`) are enumerated in
`docs/language-bad-behavior.md`. The rule is binary: every match in
product code is either (a) renamed to a concrete domain noun, (b) replaced
with a typed `Result<T, E>` return where the marker was hiding an error
path, or (c) deleted along with the dead code it described. Generated
zones and intentional carve-outs are listed in
`.jankurai/generated-zones.toml`.

## Rerun the audit

```
jankurai audit . --mode advisory \
  --json .jankurai/repo-score.json --md .jankurai/repo-score.md
```

Compare the new score line in `.jankurai/repo-score.md` against the
preceding entry in `.jankurai/score-history.csv` to confirm motion in the
expected direction.
