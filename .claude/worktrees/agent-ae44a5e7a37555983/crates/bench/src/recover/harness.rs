use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::config::{
    DurabilityKind, EngineKind, RecoverChildArgs, RecoveryMatrixCase, RecoveryScenarioKind,
    RunSpec, WorkloadKind,
};
use crate::engine::{self, CellValue, engine_name};

use super::units::{
    commit_recovery_unit, recovery_catalog_unit, recovery_checkpoint_unit, recovery_wal_unit,
};
use super::{RecoveryMatrixRun, RecoveryRun};

pub fn run_single_recovery(
    engine: EngineKind,
    durability: DurabilityKind,
    scenario: RecoveryScenarioKind,
    seconds: u64,
    rows: usize,
    checkpoint_every_rows: usize,
) -> Result<RecoveryRun> {
    let tmp = tempfile::tempdir().context("create single recovery tempdir")?;
    let db_dir = tmp.path().join(format!("{engine:?}-{scenario:?}"));
    let ack_log = tmp.path().join(format!("{engine:?}-{scenario:?}.ack"));
    let child_args = RecoverChildArgs {
        engine,
        durability,
        scenario,
        db_dir: db_dir.clone(),
        ack_log: ack_log.clone(),
        rows,
        checkpoint_every_rows,
    };
    let mut child = spawn_recovery_child(&child_args)?;
    thread::sleep(Duration::from_secs(seconds.max(1)));
    stop_child(&mut child)?;

    let acknowledged = read_ack_count(&ack_log)?;
    let recovered = verify_recovered(engine, durability, scenario, &db_dir)?;
    Ok(RecoveryRun {
        engine,
        durability,
        scenario,
        acknowledged,
        recovered,
        passed: recovered >= acknowledged,
    })
}

pub fn run_matrix_case(
    engine: EngineKind,
    durability: DurabilityKind,
    case: &RecoveryMatrixCase,
    kill_after_ms: u64,
) -> Result<RecoveryMatrixRun> {
    let tmp = tempfile::tempdir()
        .with_context(|| format!("create recovery matrix tempdir for case {}", case.name))?;
    let db_dir = tmp.path().join(format!("{engine:?}-{}", case.name));
    let ack_log = tmp.path().join(format!("{engine:?}-{}.ack", case.name));
    let child_args = RecoverChildArgs {
        engine,
        durability,
        scenario: case.scenario,
        db_dir: db_dir.clone(),
        ack_log: ack_log.clone(),
        rows: case.rows.max(1),
        checkpoint_every_rows: case.checkpoint_every_rows.max(1),
    };
    let mut child = spawn_recovery_child(&child_args)?;
    thread::sleep(Duration::from_millis(kill_after_ms.max(1)));
    stop_child(&mut child)?;

    let acknowledged = read_ack_count(&ack_log)?;
    let recovered = verify_recovered(engine, durability, case.scenario, &db_dir)?;
    Ok(RecoveryMatrixRun {
        case: case.name.clone(),
        engine,
        durability,
        scenario: case.scenario,
        kill_after_ms,
        acknowledged,
        recovered,
        passed: recovered >= acknowledged,
    })
}

fn spawn_recovery_child(args: &RecoverChildArgs) -> Result<Child> {
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .arg("recover-child")
        .arg("--engine")
        .arg(engine_name(args.engine))
        .arg("--durability")
        .arg(args.durability.as_str())
        .arg("--scenario")
        .arg(args.scenario.as_str())
        .arg("--db-dir")
        .arg(&args.db_dir)
        .arg("--ack-log")
        .arg(&args.ack_log)
        .arg("--rows")
        .arg(args.rows.to_string())
        .arg("--checkpoint-every-rows")
        .arg(args.checkpoint_every_rows.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("spawn recovery child for {:?}", args.engine))
}

fn stop_child(child: &mut Child) -> Result<()> {
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn verify_recovered(
    engine: EngineKind,
    durability: DurabilityKind,
    scenario: RecoveryScenarioKind,
    db_dir: &Path,
) -> Result<usize> {
    let spec = RunSpec {
        engine,
        workload: WorkloadKind::SingleRowInsert,
        durability,
        threads: 1,
        rows: 1,
        duration: Duration::from_secs(1),
        cache_bytes: 8 * 1024 * 1024,
        seed: 7,
        base_dir: db_dir.parent().unwrap_or(db_dir).to_path_buf(),
    };
    let engine = engine::open(&spec, db_dir)
        .with_context(|| format!("open recovered engine for {scenario:?}"))?;
    let mut conn = engine
        .connect(0)
        .with_context(|| format!("connect recovered engine for {scenario:?}"))?;
    let recovered = match scalar_i64(&mut *conn, "SELECT COUNT(*) FROM crash_progress") {
        Ok(v) => v,
        Err(err) => {
            let lowered = err.to_string().to_ascii_lowercase();
            if lowered.contains("not found")
                || lowered.contains("no such table")
                || lowered.contains("missing database")
            {
                0
            } else {
                return Err(err);
            }
        }
    };
    let _ = scalar_i64(&mut *conn, "SELECT COUNT(*) FROM sqlite_schema");
    if matches!(scenario, RecoveryScenarioKind::Checkpoint) {
        engine.checkpoint()?;
    }
    if recovered < 0 {
        bail!("negative recovery count after reopen");
    }
    Ok(recovered as usize)
}

fn read_ack_count(path: &Path) -> Result<usize> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    Ok(contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count())
}

fn scalar_i64(conn: &mut dyn engine::BenchConn, sql: &str) -> Result<i64> {
    let row = conn.query_row(sql, &[])?;
    match row.first() {
        Some(CellValue::Integer(value)) => Ok(*value),
        Some(CellValue::Null) | None => Ok(0),
        other => bail!("expected integer scalar, got {other:?}"),
    }
}

pub fn default_matrix_checkpoint_every_rows() -> usize {
    32
}
