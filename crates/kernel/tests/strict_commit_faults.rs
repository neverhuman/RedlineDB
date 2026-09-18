#![cfg(feature = "failpoints")]
//! P2-SAFE safety acceptance, currently failing: caught commit panics must fence
//! continued engine use. This is a panic reproducer, NOT a returned fsync error.
use redlinedb_kernel::engine::{CommitDurability, Engine, EngineConfig};
use redlinedb_kernel::format::RowId;
use redlinedb_kernel::txn::Isolation;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn strict_commit_panic_requires_recovery_before_continued_use() {
    const CHILD_DB: &str = "REDLINE_STRICT_FAULT_CHILD_DB";
    if let Some(path) = std::env::var_os(CHILD_DB) {
        let _scenario = fail::FailScenario::setup();
        let engine = Engine::create(
            path,
            EngineConfig {
                busy_timeout: Duration::from_millis(20),
                commit_durability: CommitDurability::Strict,
                ..EngineConfig::default()
            },
        )
        .unwrap();
        let mut initial = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .insert_with_row_id(&mut initial, RowId(42), b"old".to_vec())
            .unwrap();
        engine.commit(initial).unwrap();
        let mut writer = engine.begin(Isolation::Snapshot).unwrap();
        engine
            .update(&mut writer, RowId(42), b"new".to_vec())
            .unwrap();
        let writer_id = writer.id();
        fail::cfg("engine::commit::before_publish", "panic").unwrap();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| engine.commit(writer)));
        fail::remove("engine::commit::before_publish");
        assert!(result.is_err(), "the post-flush panic hook must fire");

        // A future fence should reject this operation. Further observations are
        // diagnostic only, collected on the currently unsafe continued-use path.
        let continued = engine.begin(Isolation::Snapshot);
        if let Ok(mut reader) = continued {
            let local = engine.get(&mut reader, RowId(42));
            engine.rollback(reader).unwrap();
            let mut contender = engine.begin(Isolation::Snapshot).unwrap();
            let competing = engine.update(&mut contender, RowId(42), b"wrong".to_vec());
            engine.rollback(contender).unwrap();
            let mut unrelated = engine.begin(Isolation::Snapshot).unwrap();
            engine
                .insert_with_row_id(&mut unrelated, RowId(43), b"later".to_vec())
                .unwrap();
            let later_commit = engine.commit(unrelated);
            let mut fresh = engine.begin(Isolation::Snapshot).unwrap();
            let later_visible = engine.get(&mut fresh, RowId(43));
            engine.rollback(fresh).unwrap();
            eprintln!(
                "P2-SAFE: panic=true; fenced=false; tx_state={:?}; local={local:?}; \
                 competing={competing:?}; later_commit={later_commit:?}; \
                 later_visible={later_visible:?}; stats={:?}",
                engine.tx_state(writer_id),
                engine.tx_status_stats(),
            );
            // Fail the safety assertion, while bypassing destructors so the
            // parent can examine recovery of the exact durable WAL state.
            std::process::exit(42);
        }
        std::process::exit(0);
    }

    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "strict_commit_panic_requires_recovery_before_continued_use",
            "--nocapture",
        ])
        .env(CHILD_DB, temp.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("P2-SAFE child exceeded 20-second deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    let recovered = Engine::open(temp.path(), EngineConfig::default()).unwrap();
    let mut reader = recovered.begin(Isolation::Snapshot).unwrap();
    let value = recovered.get(&mut reader, RowId(42)).unwrap();
    let later = recovered.get(&mut reader, RowId(43)).unwrap();
    recovered.rollback(reader).unwrap();
    eprintln!("P2-SAFE recovery: uncertain_row={value:?}; later_row={later:?}");
    assert_eq!(
        value,
        Some(b"new".to_vec()),
        "hook is after successful Strict flush"
    );
    assert!(
        output.status.success(),
        "caught post-flush commit panic allowed continued engine use; \
         require a recovery fence (child status {})",
        output.status
    );
}
