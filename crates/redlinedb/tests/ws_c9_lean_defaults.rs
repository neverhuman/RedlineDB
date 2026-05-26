//! WS-C9 / Wave-6b: lean ephemeral defaults.
//!
//! Short-lived `:memory:` / CLI / smoke-test databases used to pay for a
//! 16 MB buffer pool and a 32-slot statement cache by default. The
//! `lean_ephemeral` flavor shrinks the buffer pool to 256 pages (1 MB at
//! 4 KB) and the statement cache to 8 slots. Wave-6b promoted the
//! flavor to default-on for `Database::create_in_memory` and
//! `Database::create_ephemeral` (file-backed opens keep the historical
//! sizing). An explicit `with_lean_ephemeral(false)` overrides the
//! auto-default.

use redlinedb::{Database, LEAN_BUFFER_POOL_PAGES, LEAN_STATEMENT_CACHE_CAPACITY, OpenOptions};

#[test]
fn default_in_memory_open_is_lean_by_default() {
    // Wave-6b: `:memory:` opens auto-enable the lean buffer pool.
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    assert_eq!(
        db.buffer_pool_pages(),
        LEAN_BUFFER_POOL_PAGES,
        "default in-memory open should be lean post-Wave-6b"
    );
}

#[test]
fn explicit_false_overrides_in_memory_auto_default() {
    // Callers can still opt out of the auto-lean default and get the
    // pre-Wave-6b buffer-pool size.
    let opts = OpenOptions::default().with_lean_ephemeral(false);
    let db = Database::create_in_memory(opts).expect("create db");
    assert!(
        db.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES,
        "with_lean_ephemeral(false) must keep the historical pool size, got {}",
        db.buffer_pool_pages()
    );
}

#[test]
fn lean_ephemeral_shrinks_buffer_pool_to_one_meg() {
    let opts = OpenOptions::default().with_lean_ephemeral(true);
    let db = Database::create_in_memory(opts).expect("create db");
    assert_eq!(
        db.buffer_pool_pages(),
        LEAN_BUFFER_POOL_PAGES,
        "lean_ephemeral must force the small buffer pool"
    );
}

#[test]
fn lean_ephemeral_on_disk_open_honours_the_flavor() {
    // The on-disk path uses `sql_options` directly (no clamp), so the
    // default would have been 4096 pages. Lean shrinks it to 256.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("lean.redline");
    let opts = OpenOptions::default().with_lean_ephemeral(true);
    let db = Database::open_with_options(&path, opts).expect("open");
    assert_eq!(db.buffer_pool_pages(), LEAN_BUFFER_POOL_PAGES);
}

#[test]
fn lean_setting_does_not_change_other_databases() {
    // Two ephemeral databases under distinct session names: one with
    // explicit lean=false, one default (auto-lean). Each must see its
    // own buffer-pool size. Confirms the option is per-`Database`, not a
    // process-global toggle.
    let db_default =
        Database::create_ephemeral("ws-c9-lean-iso-a", OpenOptions::default()).expect("db default");
    let db_fat = Database::create_ephemeral(
        "ws-c9-lean-iso-b",
        OpenOptions::default().with_lean_ephemeral(false),
    )
    .expect("db fat");
    assert_eq!(db_default.buffer_pool_pages(), LEAN_BUFFER_POOL_PAGES);
    assert!(db_fat.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES);
}

#[test]
fn lean_constant_matches_one_megabyte_at_four_kib_pages() {
    // Guard against future refactors that change the constant without
    // updating the docs. 256 * 4096 = 1 MiB.
    assert_eq!(LEAN_BUFFER_POOL_PAGES * 4096, 1024 * 1024);
    assert!(
        LEAN_STATEMENT_CACHE_CAPACITY <= 16,
        "lean statement cache must stay small ({LEAN_STATEMENT_CACHE_CAPACITY})"
    );
}

#[test]
fn open_with_and_without_lean_uses_separate_registry_entries() {
    // Distinct session names → no fingerprint conflict. The lean
    // setting is part of `OpenFingerprint`, so two unrelated databases
    // (one default-auto-lean, one explicit-fat) coexist without
    // poisoning each other's settings.
    let lean = Database::create_ephemeral("ws-c9-coexist-lean", OpenOptions::default())
        .expect("lean (auto)");
    let fat = Database::create_ephemeral(
        "ws-c9-coexist-fat",
        OpenOptions::default().with_lean_ephemeral(false),
    )
    .expect("fat");
    assert_ne!(lean.buffer_pool_pages(), fat.buffer_pool_pages());
}
