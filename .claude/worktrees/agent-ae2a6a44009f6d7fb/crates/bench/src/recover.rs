use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::config::{
    DurabilityKind, EngineKind, RecoverArgs, RecoverChildArgs, RecoverMatrixArgs,
    RecoveryMatrixCase, RecoveryMatrixConfig, RecoveryScenarioKind, RunSpec, WorkloadKind,
};
use crate::engine::{self, CellValue};

#[derive(Debug, Serialize)]
pub struct RecoveryReport {
    pub runs: Vec<RecoveryRun>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryRun {
    pub engine: EngineKind,
    pub durability: DurabilityKind,
    pub scenario: RecoveryScenarioKind,
    pub acknowledged: usize,
    pub recovered: usize,
    pub passed: bool,
}

#[derive(Debug, Serialize)]
pub struct RecoveryMatrixReport {
    pub runs: Vec<RecoveryMatrixRun>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryMatrixRun {
    pub case: String,
    pub engine: EngineKind,
    pub durability: DurabilityKind,
    pub scenario: RecoveryScenarioKind,
    pub kill_after_ms: u64,
    pub acknowledged: usize,
    pub recovered: usize,
    pub passed: bool,
}

pub fn run(args: &RecoverArgs) -> Result<RecoveryReport> {
    let mut runs = Vec::new();
    for &engine in args.engine.expand() {
        let result = run_single_recovery(
            engine,
            args.durability,
            RecoveryScenarioKind::Wal,
            args.seconds,
            1 << 20,
            default_matrix_checkpoint_every_rows(),
        )?;
        runs.push(result);
    }
    Ok(RecoveryReport { runs })
}

pub fn run_matrix(args: &RecoverMatrixArgs) -> Result<RecoveryMatrixReport> {
    let raw = fs::read_to_string(&args.config)
        .with_context(|| format!("read recovery matrix {}", args.config.display()))?;
    let matrix = toml::from_str::<RecoveryMatrixConfig>(&raw)
        .with_context(|| format!("parse recovery matrix {}", args.config.display()))?;
    if matrix.cases.is_empty() {
        bail!("recovery matrix must define at least one case");
    }

    let mut runs = Vec::new();
    for &engine in args.engine.expand() {
        for &durability in &matrix.durabilities {
            for case in &matrix.cases {
                for &kill_after_ms in &case.kill_windows_ms {
                    runs.push(run_matrix_case(engine, durability, case, kill_after_ms)?);
                }
            }
        }
    }
    Ok(RecoveryMatrixReport { runs })
}

pub fn run_child(args: &RecoverChildArgs) -> Result<()> {
    fs::create_dir_all(&args.db_dir)?;
    let spec = RunSpec {
        engine: args.engine,
        workload: WorkloadKind::SingleRowInsert,
        durability: args.durability,
        threads: 1,
        rows: args.rows,
        duration: Duration::from_secs(1),
        cache_bytes: 8 * 1024 * 1024,
        seed: 7,
        base_dir: args.db_dir.parent().unwrap_or(&args.db_dir).to_path_buf(),
    };
    let engine = engine::open(&spec, &args.db_dir)?;
    engine.setup_schema()?;
    let mut conn = engine.connect(0)?;
    ensure_crash_schema(&mut *conn)?;
    let mut ack = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.ack_log)?;
    for key in 0..args.rows {
        commit_recovery_unit(
            engine.as_ref(),
            &mut *conn,
            args.scenario,
            key,
            args.rows,
            args.checkpoint_every_rows,
        )?;
        writeln!(ack, "{key}")?;
        ack.flush()?;
    }
    Ok(())
}

fn run_single_recovery(
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

fn run_matrix_case(
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

fn commit_recovery_unit(
    engine: &dyn engine::BenchEngine,
    conn: &mut dyn engine::BenchConn,
    scenario: RecoveryScenarioKind,
    key: usize,
    total_rows: usize,
    checkpoint_every_rows: usize,
) -> Result<()> {
    conn.begin_immediate()?;
    match scenario {
        RecoveryScenarioKind::Wal => {
            recovery_wal_unit(conn, key)?;
        }
        RecoveryScenarioKind::Catalog => {
            recovery_catalog_unit(conn, key)?;
        }
        RecoveryScenarioKind::Checkpoint => {
            recovery_checkpoint_unit(conn, key, total_rows)?;
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO crash_progress(id, scenario, note) VALUES (?1, ?2, ?3)",
        &[
            CellValue::Integer(key as i64),
            CellValue::Text(scenario.as_str().to_owned()),
            CellValue::Text(format!("ack-{key}")),
        ],
    )?;
    conn.commit()?;
    if matches!(scenario, RecoveryScenarioKind::Checkpoint)
        && checkpoint_every_rows > 0
        && key > 0
        && key.is_multiple_of(checkpoint_every_rows)
    {
        engine.checkpoint()?;
    }
    Ok(())
}

fn recovery_wal_unit(conn: &mut dyn engine::BenchConn, key: usize) -> Result<()> {
    let params = [
        CellValue::Integer(key as i64),
        CellValue::Integer((key % 32) as i64),
        CellValue::Blob(format!("value-{key:08}").into_bytes()),
        CellValue::Integer(1),
    ];
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)",
        &params,
    )?;
    Ok(())
}

fn recovery_catalog_unit(conn: &mut dyn engine::BenchConn, key: usize) -> Result<()> {
    let slot = key % 8;
    let table = format!("scratch_{slot}");
    conn.execute(
        &format!("CREATE TABLE IF NOT EXISTS {table}(id INTEGER PRIMARY KEY, note TEXT)"),
        &[],
    )?;
    conn.execute(
        &format!("CREATE INDEX IF NOT EXISTS {table}_note_idx ON {table}(note)"),
        &[],
    )?;
    conn.execute(
        &format!("INSERT INTO {table}(id, note) VALUES (?1, ?2)"),
        &[
            CellValue::Integer(key as i64),
            CellValue::Text(format!("catalog-{key}")),
        ],
    )?;
    if key.is_multiple_of(2) {
        let _ = conn.execute(&format!("DROP INDEX IF EXISTS {table}_note_idx"), &[])?;
        let _ = conn.execute(&format!("DROP TABLE IF EXISTS {table}"), &[])?;
    }
    Ok(())
}

fn recovery_checkpoint_unit(
    conn: &mut dyn engine::BenchConn,
    key: usize,
    total_rows: usize,
) -> Result<()> {
    let version = ((key % total_rows.max(1)) + 1) as i64;
    let params = [
        CellValue::Integer(key as i64),
        CellValue::Integer((key % 32) as i64),
        CellValue::Blob(format!("checkpoint-{key:08}").into_bytes()),
        CellValue::Integer(version),
    ];
    let _ = conn.execute(
        "INSERT OR REPLACE INTO kv(k, tenant, v, version) VALUES (?1, ?2, ?3, ?4)",
        &params,
    )?;
    Ok(())
}

fn ensure_crash_schema(conn: &mut dyn engine::BenchConn) -> Result<()> {
    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS crash_progress(id INTEGER PRIMARY KEY, scenario TEXT, note TEXT)",
        &[],
    )?;
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
    // Wave 7 follow-up: a child killed before its CREATE TABLE durably
    // commits leaves no `crash_progress` table on reopen. That's a valid
    // recovery outcome (zero acked → zero recovered → still passes the
    // gate), so treat "table missing" as 0 rather than propagating the
    // kernel error. Mirrors the failpoint matrix's verify_recovered.
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
    Ok(fs::read_to_string(path)
        .unwrap_or_default()
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

fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Redline => "redline",
        EngineKind::Sqlite => "sqlite",
    }
}

fn default_matrix_checkpoint_every_rows() -> usize {
    32
}
