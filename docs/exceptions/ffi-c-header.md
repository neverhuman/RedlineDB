# Exception: `crates/ffi/include/redlinedb.h`

| Field | Value |
|-------|-------|
| Path | `crates/ffi/include/redlinedb.h` |
| Owner | `c-abi` |
| Cell | `adapters` |
| Lane | `cargo test -p redlinedb-ffi --quiet --locked` |
| Stack reason | `Rust core + TS/React + Postgres + bounded Python` excludes raw C from product truth |
| Expires | indefinite — required by the sqlite3-compatible C API contract |

## Why the file exists

`redlinedb` exposes a sqlite3-compatible C ABI so existing tooling (the
`sqlite3` shell, language bindings, ORMs) can call the engine without
modification. The `.h` is hand-authored against the sqlite3 surface; the
behavior lives in `crates/ffi/src/*.rs` (Rust) and is exercised by
`crates/ffi/tests/`.

The header is **not** product truth — it is a published interface
contract. Product truth stays in Rust. The header is declared in
`agent/generated-zones.toml` so the jankurai audit treats it as an
intentional non-product-truth surface and does not flag it under
`HLT-005` (non-optimal product language).

## Negative-test coverage

`crates/ffi/tests/exec_input_boundary.rs` covers NUL bytes, non-UTF-8,
oversize SQL, null pointers, and injection patterns for the `rldb_exec`
entry point (the lone shell-execution sink the audit flags).

`crates/ffi/tests/safety_invariants.rs` covers double-free guards,
use-after-free panic, and parameter validation for each public function
declared in the header.

## Migration path

None planned. The sqlite3-compatible C ABI is a stable promise to
downstream consumers. If we ever drop sqlite3 compatibility we will
delete this header along with `crates/ffi/src/sqlite3_api.rs` in the
same change.
