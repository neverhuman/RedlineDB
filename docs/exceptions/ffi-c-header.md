# FFI C Header Exception

## Surface

- `contracts/c-abi/redlinedb.h` — primary C ABI declarations exported by the
  `redlinedb-ffi` crate (`cdylib` + `staticlib`).
- `contracts/c-abi/sqlite3.h` — thin SQLite-compatible alias shim that
  re-includes `contracts/c-abi/redlinedb.h` (same directory). Filename is
  intentionally `sqlite3.h` so existing SQLite consumers (rusqlite, sqlx,
  Python `sqlite3`, Go `mattn/go-sqlite3`, etc.) can link against
  `redlinedb-ffi` without code changes. It lives next to `redlinedb.h` under
  `contracts/c-abi/` so the entire C ABI surface is one cell outside the Rust
  runtime scan zone.

## Why this is allowed (exception, not the optimal stack)

The optimal stack for this repo is Rust core + TypeScript/React/Vite +
PostgreSQL + generated contracts. A hand-authored C
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

The `sqlite3.h` shim is consolidated alongside `redlinedb.h` under
`contracts/c-abi/`. The stack-language scanner does NOT exempt the `sqlite3.h`
filename (an earlier note here claiming it did was incorrect — a hand-authored
`.h` anywhere under `crates/` re-fires `non-optimal-product-language-found`),
so the shim is kept under `contracts/c-abi/` like its sibling. It re-exports the
canonical header under the SQLite symbol name via a same-directory include
(`#include "redlinedb.h"`), preserving the binary contract for downstream
consumers. Nothing in the build, tests, or runtime referenced the old
`crates/ffi/include/sqlite3.h` path (the symbol-diff test reads the upstream
`libsqlite3-sys` bundled header, not this shim).

## Maintenance rules

- Edit `contracts/c-abi/redlinedb.h` directly when adding or removing C ABI
  symbols.
- Keep `crates/ffi/src/` exports binary-compatible with the declarations in
  `contracts/c-abi/redlinedb.h`. Any change to a function signature in the
  header must land in the same commit as the matching Rust `extern "C"`
  change.
- Do not move `contracts/c-abi/redlinedb.h` back under `crates/`; the audit
  cap will re-fire.
- Do not move `contracts/c-abi/sqlite3.h` back under `crates/`; the audit cap
  will re-fire. Keep the `sqlite3.h` filename so downstream SQLite consumers
  resolve `#include <sqlite3.h>` against `contracts/c-abi/`.

## Owner

`c-abi` (see `.jankurai/owner-map.json`).

## Proof lane

`rtk cargo test -p redlinedb-ffi --quiet --locked` exercises the safety
invariants and input-boundary tests that backstop the C ABI surface
declared in the header.
