use super::*;
use crate::options::OpenOptions;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

/// Concurrency proof for the `OnceLock<Mutex<Registry>>` global: spawn
/// several threads that simultaneously create and reuse the same
/// ephemeral session, then assert every thread sees an `Arc` that
/// upgrades to the identical `DatabaseEntry`.
///
/// This exercises:
/// - `OnceLock::get_or_init` racing first-time initialization,
/// - `Mutex::lock` serialising registry inserts,
/// - `Weak::upgrade` deduplicating concurrent reopens,
/// - `AtomicU64` for ephemeral session-id allocation under contention.
#[test]
fn registry_concurrent_open_returns_shared_handles() {
    let session_name = format!(
        "registry-concurrent-test-{}-{}",
        std::process::id(),
        EPHEMERAL_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let options = OpenOptions {
        process_owner_lock: false,
        ..OpenOptions::default()
    };

    // First create the entry from the main thread so we have a baseline
    // pointer to compare against.
    let baseline = create_ephemeral_database(&session_name, &options)
        .expect("baseline ephemeral session creates");
    let baseline_path = baseline.path.clone();

    let mut handles = Vec::with_capacity(8);
    for _ in 0..8 {
        let name = session_name.clone();
        let opts = options.clone();
        handles.push(thread::spawn(move || {
            create_ephemeral_database(&name, &opts).expect("concurrent ephemeral reopen succeeds")
        }));
    }

    let mut entries = Vec::with_capacity(handles.len());
    for handle in handles {
        entries.push(handle.join().expect("worker thread did not panic"));
    }

    // All concurrent calls must return Arcs that point at the same
    // underlying allocation as the baseline (proves Mutex serialisation
    // and Weak::upgrade dedup work correctly under load).
    for entry in &entries {
        assert!(
            Arc::ptr_eq(&baseline, entry),
            "concurrent reopen returned a different DatabaseEntry"
        );
        assert_eq!(entry.path, baseline_path, "path must round-trip");
    }

    // Drop everything and confirm the entry is purged from the registry.
    drop(entries);
    drop(baseline);

    let reg = registry().lock().expect("registry lock");
    if let Some(weak) = reg.entries.get(&baseline_path) {
        assert!(
            weak.upgrade().is_none(),
            "registry entry should be a dangling Weak once owners drop"
        );
    }
}

/// Confirm that distinct session names produce distinct registry
/// entries even when many threads race to create them simultaneously.
#[test]
fn registry_concurrent_distinct_sessions_are_independent() {
    let options = OpenOptions {
        process_owner_lock: false,
        ..OpenOptions::default()
    };
    let mut handles = Vec::with_capacity(8);
    for idx in 0..8 {
        let opts = options.clone();
        handles.push(thread::spawn(move || {
            let name = format!(
                "registry-distinct-test-{}-{}-{}",
                std::process::id(),
                idx,
                EPHEMERAL_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
            );
            create_ephemeral_database(&name, &opts).expect("distinct ephemeral creates succeed")
        }));
    }

    let mut paths = Vec::new();
    let mut entries = Vec::new();
    for handle in handles {
        let entry = handle.join().expect("worker thread did not panic");
        paths.push(entry.path.clone());
        entries.push(entry);
    }

    paths.sort();
    let original_len = paths.len();
    paths.dedup();
    assert_eq!(
        paths.len(),
        original_len,
        "each distinct session name must map to a distinct registry path"
    );
}

#[test]
fn registry_open_locks_are_scoped_per_path() {
    let mut registry = registry().lock().expect("registry lock");
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    let path_a = PathBuf::from(format!("/tmp/redlinedb-open-lock-a-{suffix}"));
    let path_b = PathBuf::from(format!("/tmp/redlinedb-open-lock-b-{suffix}"));

    let lock_a = open_lock_for_path(&mut registry, &path_a);
    let lock_a_again = open_lock_for_path(&mut registry, &path_a);
    let lock_b = open_lock_for_path(&mut registry, &path_b);

    assert!(
        Arc::ptr_eq(&lock_a, &lock_a_again),
        "same path should reuse the same open lock"
    );
    assert!(
        !Arc::ptr_eq(&lock_a, &lock_b),
        "distinct paths must not share an open lock"
    );
}

#[test]
fn volatile_session_path_keeps_explicit_root() {
    let root = PathBuf::from(format!(
        "/tmp/redlinedb-explicit-root-{}-{}",
        std::process::id(),
        EPHEMERAL_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let path = ephemeral_session_path(Some(root.as_path()), "explicit-root-session");

    assert!(
        path.starts_with(&root),
        "explicit OpenOptions::temp_dir must own the volatile root"
    );
}

#[test]
fn volatile_root_from_unusable_candidate_uses_process_scratch() {
    let root = tempfile::tempdir().expect("scratch dir");
    let file_path = root.path().join("not-a-directory");
    std::fs::write(&file_path, b"x").expect("probe file");

    assert_eq!(
        volatile_root_from_candidate(&file_path),
        std::env::temp_dir()
    );
}

#[cfg(unix)]
#[test]
fn volatile_root_from_existing_unwritable_candidate_uses_process_scratch() {
    let root = tempfile::tempdir().expect("scratch dir");
    let candidate = root.path().join("existing-shared-root");
    std::fs::create_dir(&candidate).expect("candidate dir");
    std::fs::set_permissions(&candidate, Permissions::from_mode(0o555))
        .expect("candidate permissions");

    assert_eq!(
        volatile_root_from_candidate(&candidate),
        std::env::temp_dir(),
        "an existing directory without write authority must not be selected"
    );
    assert_eq!(
        std::fs::read_dir(&candidate)
            .expect("candidate remains readable")
            .count(),
        0,
        "failed probes must leave no child custody"
    );

    std::fs::set_permissions(&candidate, Permissions::from_mode(0o700))
        .expect("restore candidate permissions");
}
