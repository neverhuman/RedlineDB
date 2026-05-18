# Fuzz Parity Grammar Generator Exception

## Surface

- `crates/bench/src/fuzz/sqlsmith.rs` — SQLSmith-style grammar fuzzer
  that emits random well-formed SQL biased toward the feature surface
  documented in `docs/sqlite-parity.md`.
- `crates/bench/src/fuzz/normalize.rs` — outcome normalization (row-set
  sort, float epsilon comparison, error-class taxonomy) used by the
  differential parity harness.

## Why this is allowed (exception, not the optimal stack)

A SQL fuzzer's sole purpose is to emit string-SQL. The
HLT-023-INPUT-BOUNDARY-GAP detector pattern-matches `string sql` against
any `format!(...)` builder, which is the correct rule for product code
but is a forcing function the fuzzer's structural type cannot satisfy.

We keep the SQL-builder pattern in `crates/bench/src/fuzz/sqlsmith.rs`
because:

1. The output is consumed by `crates/bench/tests/fuzz_parity.rs` which
   runs the generated SQL against two fresh in-memory engines (rusqlite
   and RedlineDB) per iteration. The SQL never reaches production code,
   never opens a tenant database, never crosses a trust boundary.
2. The fuzzer is the *gate* against parity drift, not a downstream
   consumer of one. Replacing `format!()` with parameterized AST nodes
   would simply move the same string-emission a layer down (the engines
   parse text).
3. Both target engines run against fresh in-memory databases that are
   discarded at the end of every test. Even a worst-case generator
   producing a malicious payload (it does not) would only affect a
   bounded heap region for the duration of one test.

## Relocation rationale

The fuzzer lives next to its harness (`crates/bench/src/fuzz/` ↔
`crates/bench/tests/fuzz_parity.rs`) so reviewers see the generator and
its proof in the same crate. Moving it to a tools/ crate would split
the gate across two compilation units for no benefit.

## Owner / Expiry

- Owner: `sqlite-parity-fuzz-d7` (see `agent/owner-map.json`).
- Expiry: never — fuzzers structurally cannot avoid string emission.
- Migration path: none required. If a future audit rule for
  fuzzer-specific patterns lands, swap this path-level exception for
  the per-rule waiver.
- Proof lane: `just fuzz-parity` exercises the generator end-to-end
  against the rusqlite oracle on every PR.
