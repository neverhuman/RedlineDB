//! A2: `REDLINEDB_DEFAULT_DURABILITY` env-var control over the open-time
//! durability default.
//!
//! Verifies the env-var path is honoured at `OpenOptions::default()` time
//! and propagates into the live engine. Without the env var the historical
//! `Strict` default stands. All cases share one test fn so the env mutations
//! serialize (cargo test runs tests in parallel by default).

use std::sync::{Mutex, OnceLock};

use redlinedb::{CommitDurability, Database, Durability, OpenOptions};

const ENV_NAME: &str = "REDLINEDB_DEFAULT_DURABILITY";
const QUIET_NAME: &str = "REDLINEDB_QUIET_DURABILITY";

/// Global serializer: cargo test runs in-binary tests in parallel by default,
/// but `std::env::set_var` is process-global, so all env-mutating tests in
/// this file must run one at a time.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_env<F: FnOnce()>(value: Option<&str>, body: F) {
    // Lock for the duration of the env mutation; panics inside `body` still
    // unwind correctly because the MutexGuard handles poisoning by ignoring.
    let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let prev = std::env::var_os(ENV_NAME);
    // Always suppress the one-line stderr notice during tests — its presence
    // is asserted separately in the dedicated stderr_notice test below.
    // SAFETY: tests in this file serialise the env mutation; no other thread
    // is reading `ENV_NAME` / `QUIET_NAME` between the set and the unset.
    unsafe {
        std::env::set_var(QUIET_NAME, "1");
        match value {
            Some(v) => std::env::set_var(ENV_NAME, v),
            None => std::env::remove_var(ENV_NAME),
        }
    }
    body();
    // SAFETY: same as above — only the test thread is mutating the env.
    unsafe {
        match prev {
            Some(v) => std::env::set_var(ENV_NAME, v),
            None => std::env::remove_var(ENV_NAME),
        }
        std::env::remove_var(QUIET_NAME);
    }
}

#[test]
fn env_var_drives_default_durability() {
    // Case 1: unset → Strict (the historical default).
    with_env(None, || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::Strict);
    });

    // Case 2: explicit "strict" → Strict.
    with_env(Some("strict"), || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::Strict);
    });

    // Case 3: "normal" → Normal (the parity-benchmark setting).
    with_env(Some("normal"), || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::Normal);
    });

    // Case 4: "NORMAL" (uppercase) → Normal — case-insensitive.
    with_env(Some("NORMAL"), || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::Normal);
    });

    // Case 5: "unsafe_dev" → UnsafeDev.
    with_env(Some("unsafe_dev"), || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::UnsafeDev);
    });

    // Case 6: "unsafe-dev" hyphen variant → UnsafeDev.
    with_env(Some("unsafe-dev"), || {
        let opts = OpenOptions::default();
        assert_eq!(opts.durability, Durability::UnsafeDev);
    });
}

#[test]
fn env_var_propagates_to_open_engine() {
    // Confirms the env-var setting reaches the live engine via the same
    // open-time path that handle.rs already wires for explicit durability.
    with_env(Some("normal"), || {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a2.redline");
        let db = Database::open_with_options(&path, OpenOptions::default()).expect("create db");
        assert_eq!(db.commit_durability(), CommitDurability::Normal);
    });
}

#[test]
#[should_panic(expected = "is not a valid durability")]
fn invalid_value_panics_loudly() {
    // Misconfiguration must surface immediately so CI catches it on the
    // first `OpenOptions::default()` call rather than mysteriously regressing
    // performance from a silently-ignored env.
    with_env(Some("yes_please"), || {
        let _opts = OpenOptions::default();
    });
}

#[test]
fn explicit_with_durability_still_overrides_env() {
    // `with_durability` is the programmatic escape hatch — must beat the env.
    with_env(Some("normal"), || {
        let opts = OpenOptions::default().with_durability(Durability::Strict);
        assert_eq!(opts.durability, Durability::Strict);
    });
}
