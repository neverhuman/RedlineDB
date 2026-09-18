# P2-ABI: upstream-header qualification and Rust implementation map

Date: 2026-09-18. Base: `af20826311a067a51383c382013612545156df61` plus
preserved dirty work. Implementer: ABI agent; independent reviewer: root pending.
Status: **reproduced, not fixed**. No production ABI or release identity changed.

## Executable qualification

```sh
rtk proxy bash scripts/compatibility/phase2-abi-probe.sh
```

Exit **1**: nine SQLite controls pass; Redline has **two passes and seven failures**.
Known defects remain failures, including crashes; no case is ignored or inverted.
The command compiles `crates/ffi/tests/phase2_abi_probe.c` with the qualified
SQLite 3.53.1 upstream header, checks the oracle header/library hashes against its
qualification receipt, and executes all controls before building Redline.
It then builds `redlinedb-ffi` from this checkout with `cargo build --locked` and
tests `target/debug/libredlinedb.so`. This is Linux debug qualification, not a
packaged release, macOS, sanitizer, threading, or complete consumer qualification.

Every function pointer has its type derived directly from the upstream declaration.
The fixture opens the absolute library with `dlopen`, checks every required
symbol's `dladdr` path against that library, and prints the loaded path before
running the probe. No SQLite library is linked into the probe. System SQLite
cannot silently substitute for either candidate. The Python runner records
library hashes, compiler/command, source/probe/script hashes, parent commit,
dirty status, platform and full oracle identity.

Each case gets a separate process and fresh temporary database. Positive bounded
and zero-length reads use an inaccessible guard page. The runner limits each
probe to 15 seconds and each output file to 64 KiB, disables core dumps, kills
the process group on timeout, and retains stdout/stderr plus signal classification.
Missing symbols or wrong loaded provenance are infrastructure failures. Oracle
failure stops before Redline and returns exit 2. The test inputs intentionally
use on-disk paths to avoid conflating preparation with the known :memory: gap.

| Case | SQLite 3.53.1 | Redline observed |
|---|---|---|
| `v3-zero` | success; row 7 and correct tail | rc 21 (MISUSE); statement output unchanged |
| `v3-persistent` | success with unsigned PERSISTENT flag | SIGABRT (misaligned output-pointer dereference) |
| `bounded-guard` | reads exactly 8 bytes of nonterminated SELECT 7 | SIGSEGV |
| `zero-guard` | no statement, no input read; tail equals input | SIGSEGV |
| `empty-tail` | success, NULL statement, tail at end of comments | pass |
| `embedded-nul` | prepares prefix; tail at embedded NUL | rc 1; no correct tail |
| `error-output` | SQLITE_ERROR and cleared statement output | rc 1; output still sentinel |
| `v2-tail` | first statement only; tail points to second | pass |
| `type-tags` | NULL=5, INTEGER=1, FLOAT=2, TEXT=3, BLOB=4 | NULL=0, FLOAT=1; other three match |

The incorrect PERSISTENT call aborts only its child; the runner still executes
all later probes. Type checks occur before any conversion accessor. A successful
v3 or v2 preparation also verifies step/finalize/close; a failed assertion exits
that isolated process without using the possibly invalid returned handle.

## Concrete Rust change map

1. **ABI-01a, v5 release boundary first.** In
   `crates/ffi/src/sqlite3_api/core.rs`, change only the SQLite function to the
   upstream order `(db, sql, nbytes, flags: c_uint, out_stmt, tail)`. Update
   `contracts/c-abi/redlinedb.h`, the wrong-order direct Rust fixture in
   `crates/ffi/src/tests.rs`, and all call sites together. Preserve native
   `rldb_*` interfaces unless separately reviewed. The upstream fixture must
   pass both zero and nonzero flags through the exported dynamic symbol.
2. **ABI-01b, bounded preparation.** In `crates/ffi/src/stmt.rs`, initialize
   statement output before error paths; handle zero length without dereferencing
   input; for positive length create only the specified slice and stop at its
   first NUL; use `CStr::from_ptr` only for negative lengths. Carry consumed byte
   offsets relative to the original caller buffer when producing tails. Keep
   null output-pointer validation and error recording. The current unconditional
   `CStr::from_ptr` violates its own bounded-buffer safety comment. Review shared
   native `rldb_prepare_v2` callers before choosing a shared helper or SQLite-only
   wrapper. Add error-offset, UTF-8 boundary, empty, comments, invalid SQL and
   every prefix-length regression; avoid altering parser files in this fix.
3. **ABI-02a, type identity.** In `crates/ffi/src/sqlite3_api/column.rs`, map the
   actual `SqlValue` variant directly to SQLite constants. Do not infer types by
   trying conversion accessors in `crates/ffi/src/column.rs`: a real value can
   convert to integer. Keep native NULL=0 separate from SQLite NULL=5. Audit
   `sqlite3_value_type` and `sqlite3_value_numeric_type` in
   `crates/ffi/src/sqlite3_api/value.rs`, plus the incorrect `SQLITE_NULL` macro
   in the compatibility header, under the same reviewed constant contract.
4. **ABI-02b, conversions/lifetimes follow.** `crates/ffi/src/util.rs`
   (`refresh_text_cache`, `exec_value`), native and SQLite column wrappers,
   `types.rs` statement cache fields, and `sqlite3_api/value.rs` require a common
   conversion-cache lifetime contract. Current numeric byte counts default to 8,
   CString text cannot represent embedded NUL, and NULL/empty distinctions need
   independent tests. These are source-review findings; this nine-case run does
   not claim conversion/lifetime qualification.
5. **ABI-03 prerequisite before consumer promotion.** Review the `panic=abort`
   release profile in root `Cargo.toml` and establish an unwind-capable FFI build
   with boundary catching. Do not treat catching an internal panic as a cure for
   invalid C pointer contracts. Then independently qualify dependent handles,
   destructors, allocation domains and thread modes before real consumers.

## Required v5 identity decisions

The corrected ABI must be an explicitly new **v5.0.0 prerelease**. The current
header and implementation agree on an incompatible v4 signature; replacing its
binary under an unchanged installation identity would silently break its callers.
The integration owner must select a distinct install/library ABI identity (for
example a v5 SONAME/install name and package path), an opt-in compatibility link
policy, and coordinated crate/product prerelease versions before release edits.
Preserve all v4 tags/assets and prohibit an installer from overwriting the old ABI.
Review `scripts/package-release.sh`, `scripts/test-package-ffi.sh`, and
`.github/workflows/release-build.yml` when selecting the concrete packaging path.

`sqlite3_libversion[_number]` currently reports product 4.1.0, and source ID is
`redlinedb-ffi 4.1.0`. The target contract is SQLite compatibility version 3.53.1
with a separately exposed Redline product/build identity. Decide and document
exact header/runtime compatibility version constants before consumer qualification;
never report SQLite's source hash as Redline's source identity. A pinned upstream
header plus separate Redline additions and a supported-API manifest must be part
of the same release. No release qualification follows from merely fixing symbols.

## Evidence identities

Raw receipt: `target/compatibility-phase2/abi/receipt.json`; each case has separate
`.stdout` and `.stderr` files there. It includes exact per-process invocations,
exit/signal classifications, output hashes and build log hash.

- `receipt SHA-256`: `7766a3c102d4949a220a9e721694612b2cfa1978b6b92a9e5da3ac3cb7bd961e`
- `C fixture SHA-256`: `d69a9f8a57dc1a0e319bfb56692586769a96dd210ac1f27cb880cece9643d72c`
- `runner SHA-256`: `cf5a08dc64d6795d7e5682d4a59f1c780876787fcce3b689e36cabfb7ff71087`
- `upstream sqlite3.h SHA-256`: `16d6e2f265fb13ece42329da39a865d4c540045b444d16b4bf9f7e1a7e39408f`
- `oracle library SHA-256`: `036de80c62e1a467816309dbd6e1e8cd1b003ff9969dc59da175f969aca60426`
- `Redline debug library SHA-256`: `dcda3cebbe762ceca2b101738d6b1e3b2f2598b3ec85d16cc44ca90d0c418f8e`
