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
use crate::engine::{self, CellValue, engine_name};

#[path = "recover/harness.rs"]
mod harness;
#[path = "recover/units.rs"]
mod units;

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
        let result = harness::run_single_recovery(
            engine,
            args.durability,
            RecoveryScenarioKind::Wal,
            args.seconds,
            1 << 20,
            harness::default_matrix_checkpoint_every_rows(),
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
                    runs.push(harness::run_matrix_case(
                        engine,
                        durability,
                        case,
                        kill_after_ms,
                    )?);
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
    units::ensure_crash_schema(&mut *conn)?;
    let mut ack = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.ack_log)?;
    for key in 0..args.rows {
        units::commit_recovery_unit(
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
