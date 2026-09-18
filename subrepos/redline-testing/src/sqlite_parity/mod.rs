pub mod case;
mod catalog;
mod engine;
mod memory;
mod normalize;
pub mod profile;
mod provenance;
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
    let mut provenance = provenance::Snapshot::begin(&config)?;
    let output = config.output.clone();
    let result = run_with_provenance(config, &mut provenance);
    provenance.finish(
        &output,
        result.as_ref().err().map(|error| format!("{error:#}")),
    )?;
    result
}

fn run_with_provenance(
    config: RunConfig,
    provenance: &mut provenance::Snapshot,
) -> Result<RunSummary> {
    provenance.start_output(&config.output)?;
    let cases = catalog::selected_official_cases()?;
    let manifest = profile::Manifest::load()?;
    manifest.validate(&catalog::all_cases()?)?;
    let (verified, required) = manifest.counts();
    eprintln!(
        "sqlite_profile id={} inventory_complete={} requirements_verified={} requirements_required={} (separate from corpus cases)",
        manifest.profile_id, manifest.inventory_complete, verified, required
    );
    if std::env::var_os("REDLINE_TESTING_QUALIFY_PROFILE").is_some() {
        manifest.qualify(&cases)?;
    }
    let reference = engine::EngineSpec::new(engine::REFERENCE_CLI_BIN, config.reference_bin);
    let target = engine::EngineSpec::new("redlinedb", config.target_bin);
    let reference_identity = reference.binary_identity()?;
    provenance.identity("reference", &reference_identity)?;
    provenance.identity("target", &target.binary_identity()?)?;
    profile::validate_oracle(&reference_identity)?;
    runner::validate_compare_engines(&reference, &target)?;
    let capabilities = reference.sqlite_shell_capabilities()?;
    let sqlite_version = capabilities
        .as_ref()
        .map(|capabilities| capabilities.version.clone());
    // Missing target capabilities are defects, not permission to remove cases.
    // Probe errors propagate as infrastructure failures.
    let _target_capabilities = target.target_capabilities()?;
    let partition = engine::partition_cases(cases, capabilities.as_ref(), None);
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
