# RedlineDB + Jansu Integration Issues Tracker

Tracks every incompatibility, workaround, or open question encountered during
the Wave 11.B integration.

## Current Status

One high-priority integration gap is open from a user request in the dougx
Jeryu cleanup flow. The resolved items below are integration-contract or
upstream dependency notes, not active engine defects.

## Open

### R-3: Jeryu autonomy ledger cannot use `redline://` / `redlineDB://` as a SQLx drop-in yet

**Date:** 2026-05-18
**Status:** open -- high priority
**Priority:** High
**Owner:** redlinedb-sqlx / external integration
**Requested by:** user, during dougx Jeryu cleanup after rejecting
`target/jeryu/autonomy.sqlite` as the autonomy ledger name.

The user request is that Jeryu autonomy state should be backed by a RedlineDB
ledger as a 100% parity drop-in, not by an `autonomy.sqlite` file. The current
blocking gap is that the pinned Jeryu `autonomy` binary rejects
`JERYU_DATABASE_URL=redline://...` / `redlineDB://...` before profile validation;
its accepted URL schemes are currently `postgres://`, `postgresql://`, and
`sqlite:`. RedlineDB provides the `redlinedb-sqlx` integration layer and
`redline://` tests, but consuming binaries still must link that crate and call
`redlinedb_sqlx::install_default_drivers()` before the first `sqlx::AnyPool` or
`sqlx::AnyConnection`.

**Required fix:** make the Jeryu/autonomy SQLx bootstrap path install the
RedlineDB driver before opening the launch ledger, document the canonical
RedlineDB ledger URL/path, and add an integration proof that
`JERYU_DATABASE_URL=redline://.../target/jeryu/autonomy.redlineDB` (or the final
canonical URL form) passes `autonomy kill-bell status` and
`autonomy profile validate --profile sovereign_plus`.

**Proof target:** a consuming-project smoke that fails without RedlineDB driver
registration and passes with it, plus the existing `redlinedb-sqlx`
`jeryu_schema` and driver-registration tests.

## Resolved

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
