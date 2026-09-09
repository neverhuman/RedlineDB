//! `redlinedb-cli` binary: a compatibility-named alias for the primary
//! `redlinedb` binary. Both targets share the single entry point defined in
//! `crates/cli/src/main.rs` (feature-gated allocator selection plus
//! `redlinedb_cli::run()`), so the two binaries can never diverge. Pulling the
//! shared source in with `include!` keeps exactly one source of truth instead
//! of a byte-for-byte copy of `main.rs`.
include!("../main.rs");
