//! WS-C9: lean ephemeral defaults.
//!
//! Short-lived `:memory:` / CLI / smoke-test databases used to pay for a
//! 16 MB buffer pool and a 32-slot statement cache by default. The
//! `lean_ephemeral` opt-in shrinks the buffer pool to 256 pages (1 MB
//! at 4 KB) and the statement cache to 8 slots so single-query runs
//! stop paying steady-state-server overhead.
//!
//! Default behaviour MUST remain unchanged — only the opt-in flavor
//! observes the smaller numbers.

use redlinedb::{
    Database, LEAN_BUFFER_POOL_PAGES, LEAN_STATEMENT_CACHE_CAPACITY, OpenOptions,
};

#[test]
fn default_open_keeps_historical_buffer_pool_size() {
    // Default `:memory:` opens go through `private_in_memory_sql_options`
    // which clamps `buffer_pool_pages` to `[16, 1024]`. The default
    // `cache_bytes` (16 MB) / page_size (4 KB) = 4096, then clamped to
    // 1024. Anything `>= 1024` (or anything `> LEAN_BUFFER_POOL_PAGES`)
    // proves we did not silently regress non-lean callers.
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let pages = db.buffer_pool_pages();
    assert!(
        pages > LEAN_BUFFER_POOL_PAGES,
        "default in-memory open should keep its pre-WS-C9 buffer pool size, got {pages}"
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
    // Two ephemeral databases under distinct session names: one lean,
    // one default. Each must see its own buffer-pool size. Confirms the
    // option is per-`Database`, not a process-global toggle.
    let db_lean = Database::create_ephemeral(
        "ws-c9-lean-iso-a",
        OpenOptions::default().with_lean_ephemeral(true),
    )
    .expect("db lean");
    let db_default =
        Database::create_ephemeral("ws-c9-lean-iso-b", OpenOptions::default()).expect("db default");
    assert_eq!(db_lean.buffer_pool_pages(), LEAN_BUFFER_POOL_PAGES);
    assert!(db_default.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES);
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
    // setting is part of `OpenFingerprint`, so even the same name
    // opened with different lean settings would be rejected as
    // incompatible — but here we only assert that two unrelated
    // databases can coexist without poisoning each other's settings.
    let lean = Database::create_ephemeral(
        "ws-c9-coexist-lean",
        OpenOptions::default().with_lean_ephemeral(true),
    )
    .expect("lean");
    let fat = Database::create_ephemeral("ws-c9-coexist-fat", OpenOptions::default()).expect("fat");
    assert_ne!(lean.buffer_pool_pages(), fat.buffer_pool_pages());
}
