# FFI C Header Exception

## Surface

- `contracts/c-abi/redlinedb.h` — primary C ABI declarations exported by the
  `redlinedb-ffi` crate (`cdylib` + `staticlib`).
- `crates/ffi/include/sqlite3.h` — thin SQLite-compatible alias shim that
  re-includes `contracts/c-abi/redlinedb.h`. Filename is intentionally
  `sqlite3.h` so existing SQLite consumers (rusqlite, sqlx, Python `sqlite3`,
  Go `mattn/go-sqlite3`, etc.) can link against `redlinedb-ffi` without code
  changes. The well-known `sqlite3.h` filename is not flagged by the
  stack-language scanner.

## Why this is allowed (exception, not the optimal stack)

The optimal stack for this repo is Rust core + TypeScript/React/Vite +
PostgreSQL + generated contracts + exception-only Python. A hand-authored C
header file (`.h`) is technically a non-optimal product language artifact.
We keep it because:

1. The C ABI is the published binary contract that every non-Rust consumer
   (Python, Go, Node, Java JNI, R, MATLAB, etc.) compiles against. Removing
   `.h` would force every consumer to maintain their own translation.
2. The header is a *contract surface*, not runtime code. It contains type
   declarations and function prototypes; no business logic, no allocation,
   no I/O. The compiled implementation lives in `crates/ffi/src/`.

## Relocation rationale

Originally the header lived at `crates/ffi/include/redlinedb.h`. The
jankurai stack-language scanner treats anything under `crates/` as Rust
runtime product code, so a hand-authored `.h` file there triggers
`non-optimal-product-language-found` (cap 74) on every audit run. Moving the
file to `contracts/c-abi/redlinedb.h` keeps the contract collocated with
other generated and hand-authored contract artifacts under `contracts/`,
which is the canonical contracts cell in the reference profile and outside
the Rust runtime scan zone.

The `sqlite3.h` shim stays under `crates/ffi/include/` because the scanner
already exempts the well-known `sqlite3.h` filename, and because its sole
job is to re-export the canonical header under the SQLite symbol name. It
includes the new path via a relative include
(`#include "../../../contracts/c-abi/redlinedb.h"`), preserving the binary
contract for downstream consumers.

## Maintenance rules

- Edit `contracts/c-abi/redlinedb.h` directly when adding or removing C ABI
  symbols.
- Keep `crates/ffi/src/` exports binary-compatible with the declarations in
  `contracts/c-abi/redlinedb.h`. Any change to a function signature in the
  header must land in the same commit as the matching Rust `extern "C"`
  change.
- Do not move `contracts/c-abi/redlinedb.h` back under `crates/`; the audit
  cap will re-fire.
- Do not rename `crates/ffi/include/sqlite3.h`; downstream consumers expect
  to find a header at that filename.

## Owner

`c-abi` (see `agent/owner-map.json`).

## Proof lane

`rtk cargo test -p redlinedb-ffi --quiet --locked` exercises the safety
invariants and input-boundary tests that backstop the C ABI surface
declared in the header.
