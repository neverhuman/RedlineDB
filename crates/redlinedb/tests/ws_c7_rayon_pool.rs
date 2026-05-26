//! WS-C7: per-`Database` Rayon thread pool.
//!
//! Each `Database` owns its own `Arc<rayon::ThreadPool>` so future
//! intra-query parallel operators can `pool.install(|| ...)` without
//! installing a global Rayon pool. Embedders that already own a Rayon
//! pool (`redlinedb-tokio`, `redlinedb-sqlx`, application analytics
//! stacks) must observe `rayon::current_num_threads()` unchanged across
//! `Database::create` / `Database::open` calls.

use redlinedb::{Database, OpenOptions};

#[test]
fn default_pool_uses_min_num_cpus_capped_at_eight() {
    let db = Database::create_in_memory(OpenOptions::default()).expect("create db");
    let expected = num_cpus::get().min(8);
    if expected <= 1 {
        assert_eq!(db.rayon_thread_count(), 0, "single-core hosts skip the pool");
    } else {
        assert_eq!(db.rayon_thread_count(), expected, "default thread count");
    }
}

#[test]
fn explicit_single_thread_disables_pool() {
    let opts = OpenOptions::default().with_rayon_threads(Some(1));
    let db = Database::create_in_memory(opts).expect("create db");
    assert_eq!(db.rayon_thread_count(), 0, "Some(1) opts out of the pool");

    let opts0 = OpenOptions::default().with_rayon_threads(Some(0));
    let db0 = Database::create_in_memory(opts0).expect("create db");
    assert_eq!(db0.rayon_thread_count(), 0, "Some(0) opts out of the pool");
}

#[test]
fn explicit_thread_count_is_honoured() {
    let opts = OpenOptions::default().with_rayon_threads(Some(3));
    let db = Database::create_in_memory(opts).expect("create db");
    assert_eq!(db.rayon_thread_count(), 3);
}

#[test]
fn each_database_owns_its_own_pool() {
    let opts_a = OpenOptions::default().with_rayon_threads(Some(2));
    let opts_b = OpenOptions::default().with_rayon_threads(Some(4));
    let db_a = Database::create_ephemeral("ws-c7-pool-a", opts_a).expect("db a");
    let db_b = Database::create_ephemeral("ws-c7-pool-b", opts_b).expect("db b");
    assert_eq!(db_a.rayon_thread_count(), 2);
    assert_eq!(db_b.rayon_thread_count(), 4);
}

#[test]
fn pool_is_not_installed_as_rayon_global() {
    // Capture the global Rayon thread count BEFORE opening any Database.
    // The global pool is lazily initialised; touching it once here pins
    // the baseline so a later `current_num_threads()` cannot lie by
    // racing the lazy init against the Database open.
    let before = rayon::current_num_threads();

    let _db1 = Database::create_in_memory(OpenOptions::default()).expect("db 1");
    let _db2 = Database::create_in_memory(
        OpenOptions::default().with_rayon_threads(Some(2)),
    )
    .expect("db 2");

    let after = rayon::current_num_threads();
    assert_eq!(
        before, after,
        "Database constructors must not install a global Rayon pool"
    );
}
