# RedlineDB + Jansu Integration Issues Tracker

Tracks every incompatibility, workaround, or open question encountered during
the Wave 11.B integration.

## Current Status

No open RedlineDB product-code issue remains from this tracker. The resolved
items below are integration-contract or upstream dependency notes, not active
engine defects.

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
