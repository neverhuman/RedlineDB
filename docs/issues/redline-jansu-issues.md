# RedlineDB + Jansu Integration Issues Tracker

Tracks every incompatibility, workaround, or open question encountered during
the Wave 11.B integration.

## Current Status

One high-priority RedlineDB parity gap is open from a user request in the
dougx Jeryu cleanup flow: large encrypted BLOB inserts can hang instead of
behaving as a SQLite drop-in. The earlier Jeryu autonomy URL bootstrap gap is
resolved by the consuming RedlineDB/Jeryu PR cleanup recorded below.

## Open

### R-4: Large encrypted BLOB inserts can hang through the SQLx/RedlineDB path

**Date:** 2026-05-18
**Status:** open -- high priority
**Priority:** High
**Owner:** SQL execution / SQLx adapter / storage BLOB path
**Requested by:** user, during the dougx RedlineDB/Jeryu cleanup. The user
explicitly requested RedlineDB as a 100% parity drop-in and asked that any
current RedlineDB parity gap be added to the tracker.

The dougx pre-merge lane exposed a RedlineDB parity gap while using the updated
RedlineDB/Jeryu pins: `veox-bootstrap::rust_api_orchestrator_contracts`
stalls during `EnclaveRepository::submit_tasks_bulk` on the first insert into
`tasks.encrypted_payload`. The payload observed at the stall was an encrypted
BLOB of about 178 KB (`177808` bytes). SQLite treats this as a normal BLOB
insert; RedlineDB must do the same for the drop-in contract.

The consuming dougx workspace now works around the gap by spilling oversized
encrypted payloads to sidecars before inserting a small marker row, but that is
not the RedlineDB parity fix. RedlineDB should accept and round-trip this class
of BLOB through the SQLx/redline store path without hanging.

**Required fix:** add a focused RedlineDB regression for inserting and reading a
large `BYTEA`/BLOB through the same SQLx adapter path used by Jeryu/Veox, root
cause the hang, and make the insert complete with SQLite-compatible behavior.

**Proof target:** a RedlineDB SQLx integration test that creates a table with a
BLOB/BYTEA column, inserts a deterministic 178 KB payload, reads it back
byte-for-byte, and fails if the operation hangs or exceeds a tight timeout.

## Resolved

### R-3: Jeryu autonomy ledger cannot use `redline://` / `redlineDB://` as a SQLx drop-in yet

**Date:** 2026-05-18
**Status:** resolved -- consuming PRs cleaned up and validated
**Priority:** High
**Owner:** redlinedb-sqlx / external integration
**Requested by:** user, during dougx Jeryu cleanup after rejecting
`target/jeryu/autonomy.sqlite` as the autonomy ledger name.

The user request is that Jeryu autonomy state should be backed by a RedlineDB
ledger as a 100% parity drop-in, not by an `autonomy.sqlite` file. The original
blocking gap was that the pinned Jeryu `autonomy` binary rejected
`JERYU_DATABASE_URL=redline://...` / `redlineDB://...` before profile validation;
its accepted URL schemes at the time were `postgres://`, `postgresql://`, and
`sqlite:`. RedlineDB provides the `redlinedb-sqlx` integration layer and
`redline://` tests, and consuming binaries must link that crate and call
`redlinedb_sqlx::install_default_drivers()` before the first `sqlx::AnyPool` or
`sqlx::AnyConnection`.

**Resolution:** per the user-provided 2026-05-18 cleanup handoff, RedlineDB PR
#18 was rebased onto `origin/main` with the intended single commit
`3e73556fe57c32800046082cba826f17b6751284`, and Jeryu PR #10 now pins that
commit. The consuming Jeryu validation passed
`cargo check -p jeryu --locked`, and dougx validation passed
`autonomy profile validate --profile sovereign_plus` with
`JERYU_DATABASE_URL=redline:///.../target/jeryu/autonomy.redlineDB`.

### R-1: RedlineDB is not a sqlx-API drop-in

**Date:** 2026-05-16  
**Status:** resolved -- contract documented  
**Owner:** rust-public-api  
**Proof:** [README.md](../../README.md), [docs/sqlite-parity.md](../sqlite-parity.md)

RedlineDB exposes a synchronous Rust facade plus a covered C ABI shim. The
`redlinedb-tokio` wrapper makes the core connection async-friendly, but it is
not itself a SQLx bridge. Consuming projects that use `sqlx::AnyPool` need the
`redlinedb-sqlx` integration layer and must install its driver before the first
`AnyPool` or `AnyConnection` is created.

### R-2: RedlineDB requires Rust 1.95 and edition 2024

**Date:** 2026-05-16  
**Status:** resolved -- documented integration constraint  
**Owner:** workspace  
**Proof:** [Cargo.toml](../../Cargo.toml), [rust-toolchain.toml](../../rust-toolchain.toml), [README.md](../../README.md)

The workspace pins Rust 1.95 and edition 2024. Consuming projects must use a
compatible toolchain before adding RedlineDB as a dependency.

### J-1: Jansu has no tagged GitHub release

**Date:** 2026-05-16  
**Status:** resolved -- external dependency tracked  
**Owner:** external integration  
**Proof:** consuming projects should pin by commit SHA until an upstream tag is
available.

This is an upstream release-management issue, not a RedlineDB code defect.

### J-2: Jansu requires Rust 1.95

**Date:** 2026-05-16  
**Status:** resolved -- external dependency tracked  
**Owner:** external integration  
**Proof:** same toolchain constraint as R-2.

The consuming workspace must move to Rust 1.95 before jansu integration can
compile.

### J-3: Jansu integration scope decision

**Date:** 2026-05-16  
**Status:** resolved -- external integration scope  
**Owner:** external integration  
**Proof:** scope belongs to the consuming project plan.

The approved scope was webhook event dispatch only: producer to jansu topic to
consumer, with topics for jobs, pipelines, and pushes. Larger uses remain
deferred to follow-up integration work.
