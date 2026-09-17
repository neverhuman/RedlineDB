//! Wave-6b: default-enable WS-C9 lean ephemeral for `:memory:` /
//! `create_ephemeral` opens.
//!
//! These tests pin the runtime contract that fell out of promoting
//! `OpenOptions::lean_ephemeral` from `bool` to `Option<bool>`:
//!   * `Database::create_in_memory(OpenOptions::default())` is lean.
//!   * `Database::create_ephemeral(_, OpenOptions::default())` is lean.
//!   * File-backed `Database::open_with_options(_, OpenOptions::default())`
//!     keeps the historical (non-lean) sizing.
//!   * Explicit `with_lean_ephemeral(false)` overrides the auto-default
//!     for the volatile opens.

use redlinedb::{Database, LEAN_BUFFER_POOL_PAGES, OpenOptions};

#[test]
fn in_memory_default_is_lean() {
    let db = Database::create_in_memory(OpenOptions::default()).expect("create in-memory");
    assert_eq!(
        db.buffer_pool_pages(),
        LEAN_BUFFER_POOL_PAGES,
        "Wave-6b: `:memory:` default open must auto-enable lean_ephemeral"
    );
}

#[test]
fn ephemeral_default_is_lean() {
    let db = Database::create_ephemeral("wave6b-ephemeral-default", OpenOptions::default())
        .expect("create ephemeral");
    assert_eq!(
        db.buffer_pool_pages(),
        LEAN_BUFFER_POOL_PAGES,
        "Wave-6b: `create_ephemeral` default open must auto-enable lean_ephemeral"
    );
}

#[test]
fn file_backed_default_stays_non_lean() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("wave6b.redline");
    let db = Database::open_with_options(&path, OpenOptions::default()).expect("open file db");
    assert!(
        db.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES,
        "Wave-6b: on-disk default opens must keep the historical buffer-pool size, got {}",
        db.buffer_pool_pages()
    );
}

#[test]
fn explicit_false_overrides_auto_default_for_memory() {
    let opts = OpenOptions::default().with_lean_ephemeral(false);
    let db = Database::create_in_memory(opts).expect("create in-memory");
    assert!(
        db.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES,
        "Wave-6b: with_lean_ephemeral(false) must defeat the auto-default, got {}",
        db.buffer_pool_pages()
    );
}

#[test]
fn explicit_false_overrides_auto_default_for_ephemeral() {
    let opts = OpenOptions::default().with_lean_ephemeral(false);
    let db = Database::create_ephemeral("wave6b-ephemeral-explicit-fat", opts)
        .expect("create ephemeral");
    assert!(
        db.buffer_pool_pages() > LEAN_BUFFER_POOL_PAGES,
        "Wave-6b: with_lean_ephemeral(false) must defeat the auto-default on ephemeral, got {}",
        db.buffer_pool_pages()
    );
}

#[test]
fn explicit_true_still_works_for_file_backed_open() {
    // Backward compat: callers that always shipped `with_lean_ephemeral(true)`
    // for short-lived file-backed opens (smoke harnesses etc.) must still
    // see the lean buffer-pool size.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("wave6b-explicit.redline");
    let opts = OpenOptions::default().with_lean_ephemeral(true);
    let db = Database::open_with_options(&path, opts).expect("open file db");
    assert_eq!(db.buffer_pool_pages(), LEAN_BUFFER_POOL_PAGES);
}
