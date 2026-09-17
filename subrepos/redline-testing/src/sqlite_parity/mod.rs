pub mod case;
mod catalog;
mod engine;
mod memory;
mod normalize;
mod report;
mod rql_phase1;
mod runner;
mod text;

use std::path::PathBuf;

use anyhow::Result;

pub use catalog::all_cases;
pub use engine::REFERENCE_CLI_BIN;
pub use rql_phase1::{RunConfig as RqlPhase1RunConfig, rql_phase1_cases};
pub use runner::RunSummary;

pub struct RunConfig {
    pub reference_bin: PathBuf,
    pub target_bin: PathBuf,
    pub output: PathBuf,
    pub tmp_root: PathBuf,
    pub workers: usize,
    pub repetitions: usize,
    pub warmup: usize,
    pub progress: bool,
    pub memory_samples: bool,
}

pub fn run(config: RunConfig) -> Result<RunSummary> {
    let cases = catalog::selected_official_cases()?;
    let reference = engine::EngineSpec::new(engine::REFERENCE_CLI_BIN, config.reference_bin);
    let target = engine::EngineSpec::new("redlinedb", config.target_bin);
    runner::validate_compare_engines(&reference, &target)?;
    let capabilities = reference.sqlite_shell_capabilities()?;
    let sqlite_version = capabilities
        .as_ref()
        .map(|capabilities| capabilities.version.clone());
    // Probe the target binary too — cases that gate on optional SQLite
    // features (fts5, rtree, dbstat, …) are skipped when the target
    // lacks them, per the ship-contract: corpus ships what passes
    // reference self-compare; the parity sweep must not gate on target
    // engine readiness for explicitly-optional features.
    let target_capabilities = target.target_capabilities().ok();
    let partition =
        engine::partition_cases(cases, capabilities.as_ref(), target_capabilities.as_ref());
    runner::compare_cases(
        &partition.runnable,
        &partition.skipped,
        &reference,
        &target,
        &config.output,
        config.tmp_root,
        config.workers,
        config.warmup,
        config.repetitions,
        sqlite_version,
        config.progress,
        config.memory_samples,
    )
}

pub fn run_rql_phase1(config: RqlPhase1RunConfig) -> Result<RunSummary> {
    rql_phase1::run(config)
}
