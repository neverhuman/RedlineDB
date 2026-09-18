//! Acknowledged Strict commits survive process exit without engine destructors.
use redlinedb_kernel::engine::{CommitDurability, CommitOutcome, Engine, EngineConfig};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::txn::Isolation;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn acknowledged_strict_commit_survives_process_exit() {
    const CHILD_DB: &str = "REDLINE_STRICT_COMMIT_CHILD_DB";
    if let Some(path) = std::env::var_os(CHILD_DB) {
        let engine = Engine::create(
            &path,
            EngineConfig {
                commit_durability: CommitDurability::Strict,
                ..EngineConfig::default()
            },
        )
        .unwrap();
        let mut tx = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .insert_with_row_id(&mut tx, RowId(42), b"acknowledged".to_vec())
            .unwrap();
        assert!(matches!(
            engine.commit(tx).unwrap(),
            CommitOutcome::Committed(_)
        ));
        // process::exit bypasses Engine/Arc Drop, checkpoint and WAL shutdown.
        std::process::exit(0);
    }

    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "acknowledged_strict_commit_survives_process_exit",
            "--nocapture",
        ])
        .env(CHILD_DB, temp.path())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Strict commit child exceeded 20-second deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "Strict commit child failed: {status}");
    let engine = Engine::open(temp.path(), EngineConfig::default()).unwrap();
    let mut reader = engine.begin(Isolation::Snapshot).unwrap();
    assert_eq!(
        engine.get(&mut reader, RowId(42)).unwrap(),
        Some(b"acknowledged".to_vec())
    );
    engine.rollback(reader).unwrap();
}
