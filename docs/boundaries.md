# Boundaries — RedlineDB

This doc names the cross-crate edges in the workspace and records
which edges are allowed, which are forbidden, and how to verify
each edge stays clean. The machine-readable form is
`.jankurai/boundaries.toml`; this doc is the human-readable companion.

## Allowed edges (top → bottom)

```
redlinedb-cli ─┐
redlinedb-ffi ─┼─► redlinedb ─► redlinedb-sql ─► redlinedb-kernel ─► redlinedb-domain
redlinedb-server ┘
redlinedb-bench ─► {redlinedb, redlinedb-kernel, redlinedb-sql, redlinedb-ffi}
```

Each arrow is a one-way Cargo dependency declared in the
corresponding crate's `Cargo.toml`. Reverse arrows are forbidden;
the workspace `cargo build` enforces this via the type system,
and `.jankurai/boundaries.toml` annotates the policy intent so audits
can detect new violations.

## Boundary rules

1. **No back-edges**: `crates/kernel/` must not depend on
   `crates/sql/`, and `crates/sql/` must not depend on
   `crates/redlinedb/`. The build will reject this; the audit
   double-checks via path-based scanning.
2. **Domain types are leaf**: `crates/domain/` has no in-workspace
   dependencies. Anything it adds must be policy-free and
   dependency-light so upper layers can import it without cycles.
3. **FFI is one-way**: `crates/ffi/` re-exports a SQLite-shaped C
   ABI. C-side symbols keep their `sqlite3_*` names (see
   `docs/language-bad-behavior.md` for the `compat → sqlite3_api`
   convention). The C header at
   `crates/ffi/include/redlinedb.h` is listed in
   `.jankurai/generated-zones.toml` as an authored ABI surface.
4. **Bench reaches in, nothing reaches out**: `crates/bench/`
   may depend on any product crate. No product crate may depend
   on `crates/bench/`.
5. **Public Rust facade narrows the surface**: only types and
   functions exported from `crates/redlinedb/src/lib.rs` are part
   of the public Rust API. Downstream consumers must not reach
   into `redlinedb-kernel` or `redlinedb-sql` directly; doing so
   bypasses the boundary contracts.

## Errors cross boundaries via `DomainError`

When a failure crosses a crate boundary, the lower crate
escalates its typed error into
`redlinedb_domain::DomainError` (see
`crates/domain/src/error.rs`). The escalation site is the only
place a `Box<dyn Error + Send + Sync>` may appear in product
code; everywhere else, errors stay typed inside their crate.

The canonical example is
`crates/kernel/src/error.rs::Error::into_domain` for
`InvalidChecksum`. Higher crates that want to surface this
failure to a user-facing API call `into_domain()` and propagate
the resulting `DomainError`.

## Generated and exception zones

- `.jankurai/generated-zones.toml` — paths the audit must not flag as
  product source (e.g. the C header).
- `docs/exceptions/` — per-file justifications for paths that
  cannot follow the default rule, with owner, proof lane,
  expiry, and migration plan.

## Verifying boundaries

- `cargo build --workspace --locked` — catches dependency cycles
  and back-edges.
- `jankurai audit . --mode advisory` — catches path-based
  violations and reports them under the `boundaries` category.
- `just fast` — runs both as part of the default proof lane.
