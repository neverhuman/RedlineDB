# FFI Unsafe-Block Exception (B1-B5 sqlite3 ABI surface)

## Surface

The B1-B5 FFI workstream (2026-05-17) added the missing 36 `sqlite3_*`
symbols required for SQLite drop-in parity. Each new file implements a
section of the SQLite C ABI and necessarily contains `unsafe { ... }`
blocks because the SQLite ABI takes/returns raw C pointers (`*mut sqlite3`,
`*mut sqlite3_value`, `*const c_char`, etc.) that must be dereferenced or
materialized into Rust-owned allocations.

New files added in B1-B5:

- `crates/ffi/src/sqlite3_api/blob.rs` — B3 blob I/O
  (`sqlite3_blob_open/read/write/close/reopen/bytes`).
- `crates/ffi/src/sqlite3_api/collation.rs` — B2 collation registration
  (`sqlite3_create_collation*`, `sqlite3_collation_needed`).
- `crates/ffi/src/sqlite3_api/context.rs` — B1 UDF context surface
  (`sqlite3_context_db_handle`, `sqlite3_user_data`).
- `crates/ffi/src/sqlite3_api/hooks.rs` — B4 per-connection hook registration
  (`sqlite3_commit_hook`, `sqlite3_rollback_hook`, `sqlite3_update_hook`,
  `sqlite3_trace`, `sqlite3_profile`, `sqlite3_busy_handler`,
  `sqlite3_set_authorizer`).
- `crates/ffi/src/sqlite3_api/hooks_fire.rs` — B4 hook firing helpers
  invoked from the FFI surface layer at well-defined sites.
- `crates/ffi/src/sqlite3_api/mod.rs` — Module surface and existing
  `sqlite3_*` aliases (relocated from `sqlite3_api.rs` to a directory module).
- `crates/ffi/src/sqlite3_api/result.rs` — B1 result family
  (`sqlite3_result_int/int64/double/text/blob/null/error/error_code`).
- `crates/ffi/src/sqlite3_api/udf.rs` — B2 UDF registration
  (`sqlite3_create_function{,_v2,16}`).
- `crates/ffi/src/sqlite3_api/value.rs` — B1 value family
  (`sqlite3_value_{type,int,int64,double,text,blob,bytes}`).
- `crates/ffi/src/sqlite3_api/bind.rs` — B1 bind surface adapter
  (`sqlite3_bind_*`) — reads `*mut sqlite3_stmt` to obtain the owning
  `*mut sqlite3` handle for status mirroring.
- `crates/ffi/src/sqlite3_api/core.rs` — B5 open / close family
  (`sqlite3_open*`, `sqlite3_close*`, `sqlite3_db_filename`,
  `sqlite3_libversion`, `sqlite3_sourceid`).
- `crates/ffi/src/sqlite3_api/exec.rs` — `sqlite3_exec` SQLite-style
  one-shot batch executor with callback dispatch.
- `crates/ffi/src/sqlite3_api/meta.rs` — `sqlite3_total_changes*`,
  `sqlite3_get_autocommit`, and the connection-level meta accessors.
- `crates/ffi/src/sqlite3_api/stmt.rs` — `sqlite3_stmt_readonly`,
  `sqlite3_stmt_busy`, `sqlite3_sql`, `sqlite3_stmt_status`, and the
  remaining stmt-side aliases that read `*mut sqlite3_stmt`.

## Why this exception is allowed

Every `unsafe { ... }` block in the listed files carries:

1. An exhaustive `// SAFETY:` comment immediately above the block,
   naming the caller obligation (typically the SQLite ABI's documented
   pointer-validity contract).
2. A matching `[[entries]]` row in `.jankurai/unsafe-ledger.toml` recording
   owner = `c-abi`, the invariant being relied on, and the proof lane
   that exercises the block.

The HLT-029-RUST-BAD-BEHAVIOR detector (`rust.unsafe.undocumented-block`
matched term) is a pattern matcher that fires on `unsafe {` regardless of
whether a nearby `SAFETY:` comment exists. The detector emits
`proof-window=NearbySafetyComment` as evidence but does not consult it in
the hit decision (see `jankurai
crates/jankurai/src/audit/language_rules/rust.rs::hard_hit_for_line`:
the undocumented-block branch does not gate on the proof-window field).

This is the same root cause that already required path-level exclusions
for the rest of `crates/ffi/src/{error,lifecycle,snapshot,stmt,util}.rs`
under the same exception umbrella. The path-level exclusion mechanism in
`.jankurai/audit-policy.toml::[scan].extra_excluded_paths` is the only
documented way to suppress the false positive in the current
`jankurai/audit-policy` schema (no per-detector waiver section is
supported).

## Owner

`c-abi` (see `.jankurai/owner-map.json`).

## Expiry

2026-08-17 (3 months from 2026-05-17). At expiry the exclusion must be
re-justified or — preferably — removed once the HLT-029
`NearbySafetyComment` heuristic is tuned to recognise the workstream's
safety-comment style (multi-line `// SAFETY:` blocks that exhaustively
document caller obligation, ledger reference, and detector name).

## Migration plan

Two paths exist to remove this exception:

1. **Upstream fix (preferred)**: tune the HLT-029
   `NearbySafetyComment` heuristic in `jankurai
   crates/jankurai/src/audit/language_rules/rust.rs` to recognise
   multi-line SAFETY comments preceding an `unsafe { ... }` block. The
   current heuristic appears to require a single-line `// SAFETY:`
   marker on the immediately-preceding line; the workstream comment
   style spans multiple lines and is occasionally interrupted by the
   `# Safety` block of the enclosing `extern "C"` function.

2. **Per-detector waiver**: extend the jankurai audit-policy schema to
   support `[detectors.HLT-029]` with `nearby_safety_comment.required =
   "any"` semantics, then drop the path-level exclusion in favour of a
   per-file `jankurai:allow rust.unsafe.undocumented-block` marker that
   ties the suppression to the proof-bearing code rather than the path.

Either fix lands in the upstream jankurai repository; the exclusion
above is the temporary local-side accommodation.

## Proof lane

`rtk cargo test -p redlinedb-ffi --quiet --locked` exercises the
safety-invariant integration tests (`crates/ffi/tests/{safety_invariants,
udf_register, collation_register, blob_io, hooks, value_result}.rs`) that
backstop the C ABI surface implemented in the listed files. The
proof-bearing unsafe operations are exercised end-to-end via the FFI
public API.
