use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::beyond_sqlite::case::{BeyondCase, BeyondPriority};
use crate::beyond_sqlite::engine::{ResolveOutcome, resolve as resolve_postgres};
use crate::beyond_sqlite::oracle::{
    CaseOutcome, OracleSummary, RunCasesOptions, run_selected_cases_with,
};
use crate::cli::args::{RunArgs, RunMode};
use crate::sqlite_parity::case::Case;
use crate::{beyond_sqlite, sqlite_parity};

const CONTRACT_MANIFEST_PATH: &str = "contracts/compatibility-v1.toml";
const CONTRACT_MANIFEST: &str = include_str!("../contracts/compatibility-v1.toml");
const POSTGRES_EXCLUSIONS: &str = include_str!("../metadata/beyond_sqlite/skip-list.toml");
const SQLITE_CONTRACT: &str = "redline-sqlite-contract/v1";
const POSTGRES_CONTRACT: &str = "redline-postgres-contract/v1";
const EVIDENCE_SCHEMA: &str = "redline.compat-evidence/v1";
const FORBIDDEN_EXCLUSION_REASONS: [&str; 3] = ["unimplemented", "oracle unavailable", "flaky"];

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ContractManifest {
    schema_version: String,
    contract: Vec<Contract>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Contract {
    id: String,
    compatibility_mode: String,
    oracle: String,
    corpus_path: String,
    exclusions_path: Option<String>,
    expected_case_count: usize,
    required_case_count: usize,
    exclusion_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExclusionManifest {
    #[serde(default)]
    skip: Vec<Exclusion>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Exclusion {
    case_id: String,
    name: String,
    category: String,
    rationale: String,
    target_release: String,
}

#[derive(Debug, Clone)]
struct CaseDescriptor {
    id: u64,
    category: String,
    priority: String,
}

#[derive(Debug, Serialize)]
struct BinaryIdentity {
    path: String,
    sha256: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct RunnerIdentity {
    version: String,
    commit: String,
    binary: BinaryIdentity,
}

#[derive(Debug, Serialize)]
struct HostIdentity {
    os: String,
    arch: String,
    hostname: String,
    cpu: String,
    available_parallelism: usize,
}

#[derive(Debug)]
struct ReleaseIdentity {
    engine_commit: String,
    engine_tag: String,
    storage_format_version: String,
    performance_baseline_id: String,
    oracle_custody_receipt: Option<PathBuf>,
    custody_oracle_sha256: BTreeMap<String, String>,
}

#[derive(Debug)]
struct Selection {
    ids: BTreeSet<u64>,
    explanation: String,
}

pub(crate) fn run_contract(args: RunArgs) -> Result<()> {
    let manifest = load_and_validate_manifest()?;
    let contract_id = args
        .contract
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("--contract is required for contract execution"))?;
    let contract = manifest
        .contract
        .iter()
        .find(|contract| contract.id == contract_id)
        .ok_or_else(|| anyhow::anyhow!("unknown compatibility contract `{contract_id}`"))?
        .clone();
    crate::cli::run::validate_samples(args.repetitions, args.warmup)?;
    let release_identity = release_identity(args.mode)?;
    let mut target = binary_identity(&args.target_bin)?;
    if !target.version.to_ascii_lowercase().contains("redlinedb") {
        target.version = product_version(&args.target_bin)?;
    }
    if !target.version.to_ascii_lowercase().contains("redlinedb") {
        bail!(
            "compatibility target must identify as RedlineDB via --version, got `{}`",
            target.version
        );
    }
    let engine_semver = extract_semver(&target.version).ok_or_else(|| {
        anyhow::anyhow!("target version lacks canonical SemVer: {}", target.version)
    })?;
    let contract_hash = sha256_bytes(CONTRACT_MANIFEST.as_bytes());
    let corpus_hash = corpus_hash(&contract)?;
    let evidence_path = compatibility_evidence_path(&args.output)?;

    let execution = match contract.id.as_str() {
        SQLITE_CONTRACT => run_sqlite_contract(&args, &contract)?,
        POSTGRES_CONTRACT => run_postgres_contract(&args, &contract)?,
        other => bail!("contract `{other}` has no execution adapter"),
    };
    let selected_ids = execution.selected_ids();
    let selected_ids_hash = sha256_bytes(
        selected_ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    let mut failures = execution.release_failures(&contract);
    if args.mode == RunMode::Release {
        let oracle_key = if contract.id == SQLITE_CONTRACT {
            "sqlite"
        } else {
            "postgres_client"
        };
        let expected = release_identity
            .custody_oracle_sha256
            .get(oracle_key)
            .context("custody receipt is missing selected oracle identity")?;
        match execution.oracle_sha256() {
            Some(actual) if actual != expected => failures.push(format!(
                "oracle custody mismatch: expected={expected} actual={actual}"
            )),
            None => failures.push("selected oracle has no executable identity".to_owned()),
            Some(_) => {}
        }
    }
    let status = if failures.is_empty() {
        if args.mode == RunMode::Release {
            "pass"
        } else {
            "diagnostic"
        }
    } else {
        "fail"
    };
    let custody = release_identity
        .oracle_custody_receipt
        .as_ref()
        .map(|path| binary_identity_without_version(path))
        .transpose()?;
    let evidence = json!({
        "schema_version": EVIDENCE_SCHEMA,
        "status": status,
        "mode": run_mode(args.mode),
        "engine": {
            "commit": release_identity.engine_commit,
            "tag": release_identity.engine_tag,
            "semver": engine_semver,
            "binary": target,
        },
        "storage_format_version": release_identity.storage_format_version,
        "compatibility_mode": contract.compatibility_mode,
        "contract": {
            "id": contract.id,
            "manifest_path": CONTRACT_MANIFEST_PATH,
            "manifest_sha256": contract_hash,
            "corpus_path": contract.corpus_path,
            "corpus_sha256": corpus_hash,
            "total_case_count": contract.expected_case_count,
            "required_case_count": contract.required_case_count,
            "exclusion_count": contract.exclusion_count,
        },
        "selection": {
            "selector": args.cases,
            "explanation": execution.explanation(),
            "selected_case_count": selected_ids.len(),
            "expected_case_count": selected_ids.len(),
            "selected_case_ids": selected_ids,
            "selected_case_ids_sha256": selected_ids_hash,
        },
        "results": execution.results_json(),
        "release_failures": failures,
        "runner": runner_identity()?,
        "oracles": execution.oracle_identities(),
        "oracle_custody": custody,
        "host": host_identity(),
        "performance_baseline_id": release_identity.performance_baseline_id,
        "command_line": env::args().collect::<Vec<_>>(),
    });
    write_json(&evidence_path, &evidence)?;
    println!(
        "redline compatibility evidence: {}",
        evidence_path.display()
    );
    if args.mode == RunMode::Release && status != "pass" {
        bail!(
            "release compatibility contract failed; evidence={}",
            evidence_path.display()
        );
    }
    Ok(())
}

enum ContractExecution {
    Sqlite {
        selection: Selection,
        summary: sqlite_parity::RunSummary,
        oracle: BinaryIdentity,
    },
    Postgres {
        selection: Selection,
        summary: OracleSummary,
        outcomes: Vec<CaseOutcome>,
        oracle: Option<BinaryIdentity>,
        unavailable_reason: Option<String>,
    },
}

impl ContractExecution {
    fn selected_ids(&self) -> Vec<u64> {
        match self {
            Self::Sqlite { selection, .. } | Self::Postgres { selection, .. } => {
                selection.ids.iter().copied().collect()
            }
        }
    }

    fn explanation(&self) -> &str {
        match self {
            Self::Sqlite { selection, .. } | Self::Postgres { selection, .. } => {
                &selection.explanation
            }
        }
    }

    fn release_failures(&self, contract: &Contract) -> Vec<String> {
        let selected = self.selected_ids().len();
        let mut failures = Vec::new();
        match self {
            Self::Sqlite { summary, .. } => {
                if summary.total != selected {
                    failures.push(format!(
                        "count drift: selected={selected} result_total={}",
                        summary.total
                    ));
                }
                if summary.failed != 0 {
                    failures.push(format!("target failures={}", summary.failed));
                }
                if summary.skipped != 0 {
                    failures.push(format!("target skips={}", summary.skipped));
                }
                if summary.passed != selected {
                    failures.push(format!(
                        "missing passing cases: expected={selected} passed={}",
                        summary.passed
                    ));
                }
            }
            Self::Postgres {
                summary,
                outcomes,
                unavailable_reason,
                ..
            } => {
                if unavailable_reason.is_some() {
                    failures.push(format!(
                        "oracle unavailable: {}",
                        unavailable_reason.as_deref().unwrap_or("unknown")
                    ));
                }
                if summary.total != selected || outcomes.len() != selected {
                    failures.push(format!(
                        "count drift: selected={selected} oracle_total={} outcomes={}",
                        summary.total,
                        outcomes.len()
                    ));
                }
                let unique = outcomes
                    .iter()
                    .map(|outcome| outcome.case_id)
                    .collect::<BTreeSet<_>>();
                if unique.len() != outcomes.len() {
                    failures.push("duplicate result case".to_owned());
                }
                if summary.failed != 0 {
                    failures.push(format!("oracle failures={}", summary.failed));
                }
                if summary.skipped_unavailable != 0 || summary.skipped_feature_missing != 0 {
                    failures.push(format!(
                        "oracle skips={}",
                        summary.skipped_unavailable + summary.skipped_feature_missing
                    ));
                }
                if summary.passed != selected {
                    failures.push(format!(
                        "missing oracle cases: expected={selected} passed={}",
                        summary.passed
                    ));
                }
                if summary.target_total != selected {
                    failures.push(format!(
                        "missing target cases: expected={selected} target_total={}",
                        summary.target_total
                    ));
                }
                if summary.target_failed != 0 || summary.target_skipped != 0 {
                    failures.push(format!(
                        "target failures={} skips={}",
                        summary.target_failed, summary.target_skipped
                    ));
                }
                if summary.target_passed != selected {
                    failures.push(format!(
                        "missing target passes: expected={selected} passed={}",
                        summary.target_passed
                    ));
                }
                if outcomes.iter().any(|outcome| {
                    outcome.status != "passed"
                        || outcome
                            .target
                            .as_ref()
                            .is_none_or(|target| target.status != "passed")
                }) {
                    failures.push("malformed or non-passing per-case result".to_owned());
                }
            }
        }
        if self.explanation().starts_with("selector=all")
            && selected != contract.required_case_count
        {
            failures.push(format!(
                "full selection drift: contract_required={} selected={selected}",
                contract.required_case_count
            ));
        }
        failures
    }

    fn results_json(&self) -> serde_json::Value {
        match self {
            Self::Sqlite { summary, .. } => json!({
                "total": summary.total,
                "passed": summary.passed,
                "failed": summary.failed,
                "skipped": summary.skipped,
                "elapsed_ns": summary.elapsed.as_nanos(),
            }),
            Self::Postgres { summary, .. } => json!({
                "total": summary.total,
                "oracle_passed": summary.passed,
                "oracle_failed": summary.failed,
                "oracle_skipped_unavailable": summary.skipped_unavailable,
                "oracle_skipped_feature_missing": summary.skipped_feature_missing,
                "target_total": summary.target_total,
                "target_passed": summary.target_passed,
                "target_failed": summary.target_failed,
                "target_skipped": summary.target_skipped,
            }),
        }
    }

    fn oracle_identities(&self) -> serde_json::Value {
        match self {
            Self::Sqlite { oracle, .. } => json!([oracle]),
            Self::Postgres {
                oracle,
                unavailable_reason,
                ..
            } => match oracle {
                Some(oracle) => json!([oracle]),
                None => json!([{
                    "status": "unavailable",
                    "reason": unavailable_reason,
                }]),
            },
        }
    }

    fn oracle_sha256(&self) -> Option<&str> {
        match self {
            Self::Sqlite { oracle, .. } => Some(&oracle.sha256),
            Self::Postgres { oracle, .. } => oracle.as_ref().map(|oracle| oracle.sha256.as_str()),
        }
    }
}

fn run_sqlite_contract(args: &RunArgs, contract: &Contract) -> Result<ContractExecution> {
    let all = sqlite_parity::all_cases()?;
    validate_unique_ids(
        all.iter().map(|case| case.id as u64),
        "SQLite compatibility corpus",
    )?;
    if all.len() != contract.expected_case_count
        || contract.required_case_count != contract.expected_case_count
    {
        bail!(
            "SQLite contract count drift: manifest total={} required={} corpus={}",
            contract.expected_case_count,
            contract.required_case_count,
            all.len()
        );
    }
    let descriptors = all.iter().map(sqlite_descriptor).collect::<Vec<_>>();
    let selection = select_cases(&args.cases, &descriptors, contract)?;
    let selected = all
        .into_iter()
        .filter(|case| selection.ids.contains(&(case.id as u64)))
        .collect::<Vec<_>>();
    crate::cli::run::prepare_output(&args.output)?;
    let tmp_root = crate::cli::run::resolve_tmp_root(&args.tmp_root)?;
    fs::create_dir_all(&tmp_root)?;
    let sqlite_bin = crate::cli::run::resolve_sqlite_bin(&args.sqlite_bin);
    let oracle = binary_identity(&sqlite_bin)?;
    let summary = sqlite_parity::run_selected(
        sqlite_parity::RunConfig {
            reference_bin: sqlite_bin,
            target_bin: args.target_bin.clone(),
            output: args.output.clone(),
            tmp_root,
            workers: crate::cli::run::resolve_workers(&args.workers)?,
            repetitions: args.repetitions,
            warmup: args.warmup,
            progress: crate::cli::run::progress_enabled(args.progress),
            memory_samples: args.memory_samples,
            fail_on_failure: false,
        },
        selected,
    )?;
    Ok(ContractExecution::Sqlite {
        selection,
        summary,
        oracle,
    })
}

fn run_postgres_contract(args: &RunArgs, contract: &Contract) -> Result<ContractExecution> {
    let all = beyond_sqlite::oracle::load_cases()?;
    validate_unique_ids(
        all.iter().map(|case| case.id),
        "PostgreSQL compatibility corpus",
    )?;
    if all.len() != contract.expected_case_count {
        bail!(
            "PostgreSQL contract count drift: manifest total={} corpus={}",
            contract.expected_case_count,
            all.len()
        );
    }
    let exclusions = load_and_validate_exclusions(&all, contract)?;
    let governed = all
        .into_iter()
        .filter(|case| !exclusions.contains(&case.id))
        .collect::<Vec<_>>();
    if governed.len() != contract.required_case_count {
        bail!(
            "PostgreSQL required count drift: manifest={} governed={}",
            contract.required_case_count,
            governed.len()
        );
    }
    let descriptors = governed.iter().map(postgres_descriptor).collect::<Vec<_>>();
    let selection = select_cases(&args.cases, &descriptors, contract)?;
    let selected = governed
        .into_iter()
        .filter(|case| selection.ids.contains(&case.id))
        .collect::<Vec<_>>();
    crate::cli::run::prepare_output(&args.output)?;
    let (oracle, unavailable_reason) = match resolve_postgres() {
        ResolveOutcome::Configured(reference) => (
            Some(BinaryIdentity {
                path: canonical_display(&reference.bin),
                sha256: sha256_file(&resolve_executable(&reference.bin)?)?,
                version: reference.version,
            }),
            None,
        ),
        ResolveOutcome::Unavailable { reason } => (None, Some(reason)),
    };
    let (summary, outcomes) = run_selected_cases_with(
        selected,
        RunCasesOptions {
            target_bin: Some(args.target_bin.clone()),
            target_args: args.target_args.clone(),
        },
    )?;
    write_postgres_outcomes(&args.output, &outcomes)?;
    Ok(ContractExecution::Postgres {
        selection,
        summary,
        outcomes,
        oracle,
        unavailable_reason,
    })
}

fn load_and_validate_manifest() -> Result<ContractManifest> {
    let manifest: ContractManifest = toml::from_str(CONTRACT_MANIFEST)?;
    if manifest.schema_version != "redline.compatibility-contracts/v1" {
        bail!("unknown compatibility manifest schema");
    }
    let ids = manifest
        .contract
        .iter()
        .map(|contract| contract.id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len() != manifest.contract.len()
        || ids != BTreeSet::from([SQLITE_CONTRACT, POSTGRES_CONTRACT])
    {
        bail!("compatibility manifest contract set is invalid");
    }
    for contract in &manifest.contract {
        if contract.required_case_count + contract.exclusion_count != contract.expected_case_count {
            bail!("{} count equation is invalid", contract.id);
        }
        if contract.oracle != "sqlite" && contract.oracle != "postgres" {
            bail!("{} has unknown oracle", contract.id);
        }
    }
    Ok(manifest)
}

fn load_and_validate_exclusions(
    cases: &[BeyondCase],
    contract: &Contract,
) -> Result<BTreeSet<u64>> {
    let manifest: ExclusionManifest = toml::from_str(POSTGRES_EXCLUSIONS)?;
    if manifest.skip.len() != contract.exclusion_count {
        bail!(
            "PostgreSQL exclusion count drift: contract={} file={}",
            contract.exclusion_count,
            manifest.skip.len()
        );
    }
    let by_id = cases
        .iter()
        .map(|case| (case.id, case))
        .collect::<BTreeMap<_, _>>();
    let mut ids = BTreeSet::new();
    for exclusion in manifest.skip {
        let id = exclusion
            .case_id
            .parse::<u64>()
            .with_context(|| format!("invalid exclusion case_id {}", exclusion.case_id))?;
        if !ids.insert(id) {
            bail!("duplicate PostgreSQL exclusion case {id}");
        }
        let case = by_id
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("exclusion references missing case {id}"))?;
        if case.name != exclusion.name || case.category != exclusion.category {
            bail!("exclusion identity drift for case {id}");
        }
        let rationale = exclusion.rationale.to_ascii_lowercase();
        if FORBIDDEN_EXCLUSION_REASONS
            .iter()
            .any(|forbidden| rationale.contains(forbidden))
        {
            bail!("case {id} uses a forbidden exclusion reason");
        }
        if rationale.trim().is_empty() || exclusion.target_release.trim().is_empty() {
            bail!("case {id} has an incomplete exclusion reason");
        }
    }
    Ok(ids)
}

fn select_cases(
    selector: &str,
    cases: &[CaseDescriptor],
    contract: &Contract,
) -> Result<Selection> {
    let selected = if selector == "all" {
        cases.iter().map(|case| case.id).collect::<BTreeSet<_>>()
    } else if let Some(raw) = selector.strip_prefix("id:") {
        if raw.trim().is_empty() {
            bail!("id selector is empty");
        }
        let requested = raw
            .split(',')
            .map(parse_case_id)
            .collect::<Result<BTreeSet<_>>>()?;
        let available = cases.iter().map(|case| case.id).collect::<BTreeSet<_>>();
        let missing = requested
            .difference(&available)
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            bail!("selector references missing or excluded cases: {missing:?}");
        }
        requested
    } else if let Some(category) = selector.strip_prefix("category:") {
        cases
            .iter()
            .filter(|case| case.category == category)
            .map(|case| case.id)
            .collect()
    } else if let Some(priority) = selector.strip_prefix("priority:") {
        cases
            .iter()
            .filter(|case| case.priority == priority)
            .map(|case| case.id)
            .collect()
    } else {
        bail!(
            "unsupported --cases selector `{selector}`; expected all, id:, category:, or priority:"
        );
    };
    if selected.is_empty() {
        bail!("case selector `{selector}` matched zero governed cases");
    }
    Ok(Selection {
        explanation: format!(
            "selector={selector}; contract={}; governed={}; selected={}; ordering=case-id-ascending",
            contract.id,
            cases.len(),
            selected.len()
        ),
        ids: selected,
    })
}

fn sqlite_descriptor(case: &Case) -> CaseDescriptor {
    CaseDescriptor {
        id: case.id as u64,
        category: case.category.clone(),
        priority: case.priority.to_string(),
    }
}

fn postgres_descriptor(case: &BeyondCase) -> CaseDescriptor {
    CaseDescriptor {
        id: case.id,
        category: case.category.clone(),
        priority: postgres_priority(case.priority).to_owned(),
    }
}

fn postgres_priority(priority: BeyondPriority) -> &'static str {
    match priority {
        BeyondPriority::P0 => "P0",
        BeyondPriority::P1 => "P1",
        BeyondPriority::P2 => "P2",
        BeyondPriority::P3 => "P3",
        BeyondPriority::P4 => "P4",
    }
}

fn parse_case_id(raw: &str) -> Result<u64> {
    let trimmed = raw
        .trim()
        .strip_prefix("BEYOND-CASE-")
        .unwrap_or(raw.trim());
    trimmed
        .parse::<u64>()
        .with_context(|| format!("invalid case id `{raw}`"))
}

fn validate_unique_ids(ids: impl IntoIterator<Item = u64>, label: &str) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            bail!("{label} contains duplicate case id {id}");
        }
    }
    Ok(())
}

fn release_identity(mode: RunMode) -> Result<ReleaseIdentity> {
    let required = |name: &str| -> Result<String> {
        match env::var(name) {
            Ok(value) if !value.trim().is_empty() => Ok(value),
            _ if mode == RunMode::Diagnostic => Ok("diagnostic-unbound".to_owned()),
            _ => bail!("release mode requires {name}"),
        }
    };
    let (custody, custody_oracle_sha256) = match env::var_os("REDLINE_ORACLE_CUSTODY_RECEIPT") {
        Some(path) => {
            let path = PathBuf::from(path);
            require_workspace_file(&path, "oracle custody receipt")?;
            let identities = validate_custody_receipt(&path)?;
            (Some(path), identities)
        }
        None if mode == RunMode::Diagnostic => (None, BTreeMap::new()),
        None => bail!("release mode requires REDLINE_ORACLE_CUSTODY_RECEIPT"),
    };
    Ok(ReleaseIdentity {
        engine_commit: required("REDLINE_ENGINE_COMMIT")?,
        engine_tag: required("REDLINE_ENGINE_TAG")?,
        storage_format_version: required("REDLINE_STORAGE_FORMAT_VERSION")?,
        performance_baseline_id: required("REDLINE_PERFORMANCE_BASELINE_ID")?,
        oracle_custody_receipt: custody,
        custody_oracle_sha256,
    })
}

fn validate_custody_receipt(path: &Path) -> Result<BTreeMap<String, String>> {
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if receipt
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some("redline.custody-receipt/v1")
        || receipt.get("status").and_then(|value| value.as_str()) != Some("pass")
    {
        bail!("oracle custody receipt is not a passing v1 receipt");
    }
    let lock_hash = receipt
        .get("cargo_lock_sha256")
        .and_then(|value| value.as_str())
        .context("custody receipt lacks cargo_lock_sha256")?;
    if sha256_file(&repo_root().join("Cargo.lock"))? != lock_hash {
        bail!("custody receipt Cargo.lock identity is stale");
    }
    for dependency in receipt
        .get("dependencies")
        .and_then(|value| value.as_array())
        .context("custody receipt lacks dependencies")?
    {
        validate_receipt_file_identity(dependency, "dependency archive")?;
    }
    let oracles = receipt
        .get("oracles")
        .and_then(|value| value.as_object())
        .context("custody receipt lacks oracles")?;
    let mut identities = BTreeMap::new();
    for name in ["sqlite", "postgres_client", "postgres_server"] {
        let identity = oracles
            .get(name)
            .with_context(|| format!("custody receipt lacks {name}"))?;
        identities.insert(
            name.to_owned(),
            validate_receipt_file_identity(identity, "oracle artifact")?,
        );
    }
    Ok(identities)
}

fn validate_receipt_file_identity(value: &serde_json::Value, label: &str) -> Result<String> {
    let path = PathBuf::from(
        value
            .get("path")
            .or_else(|| value.get("archive"))
            .and_then(|value| value.as_str())
            .with_context(|| format!("{label} path is missing"))?,
    );
    require_workspace_file(&path, label)?;
    let expected = value
        .get("sha256")
        .and_then(|value| value.as_str())
        .with_context(|| format!("{label} sha256 is missing"))?;
    let actual = sha256_file(&path)?;
    if actual != expected {
        bail!("{label} identity changed: {}", path.display());
    }
    Ok(actual)
}

fn require_workspace_file(path: &Path, label: &str) -> Result<()> {
    let canonical =
        fs::canonicalize(path).with_context(|| format!("resolve {label} at {}", path.display()))?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("{label} is not a physical regular file: {}", path.display());
    }
    let workspace = repo_root()
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow::anyhow!("resolve jain-split workspace root"))?
        .canonicalize()?;
    if !canonical.starts_with(&workspace) {
        bail!(
            "{label} is outside the in-tree workspace custody: {}",
            path.display()
        );
    }
    Ok(())
}

fn runner_identity() -> Result<RunnerIdentity> {
    let executable = env::current_exe()?;
    Ok(RunnerIdentity {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: git_output(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned()),
        binary: binary_identity_without_version(&executable)?,
    })
}

fn host_identity() -> HostIdentity {
    HostIdentity {
        os: env::consts::OS.to_owned(),
        arch: env::consts::ARCH.to_owned(),
        hostname: fs::read_to_string("/etc/hostname")
            .unwrap_or_else(|_| "unknown".to_owned())
            .trim()
            .to_owned(),
        cpu: fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|body| {
                body.lines()
                    .find_map(|line| line.strip_prefix("model name\t: ").map(str::to_owned))
            })
            .unwrap_or_else(|| "unknown".to_owned()),
        available_parallelism: std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1),
    }
}

fn binary_identity(path: &Path) -> Result<BinaryIdentity> {
    let executable = resolve_executable(path)?;
    let output = Command::new(&executable)
        .arg("--version")
        .output()
        .with_context(|| format!("capture version from {}", executable.display()))?;
    if !output.status.success() {
        bail!("{} --version failed", executable.display());
    }
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let version = String::from_utf8_lossy(stream).trim().to_owned();
    Ok(BinaryIdentity {
        path: canonical_display(&executable),
        sha256: sha256_file(&executable)?,
        version,
    })
}

fn binary_identity_without_version(path: &Path) -> Result<BinaryIdentity> {
    let executable = fs::canonicalize(path)?;
    Ok(BinaryIdentity {
        path: executable.display().to_string(),
        sha256: sha256_file(&executable)?,
        version: "not-applicable".to_owned(),
    })
}

fn product_version(path: &Path) -> Result<String> {
    let executable = resolve_executable(path)?;
    let output = Command::new(&executable)
        .arg("version")
        .output()
        .with_context(|| format!("capture product version from {}", executable.display()))?;
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let version = String::from_utf8_lossy(stream).trim().to_owned();
    if !output.status.success() || !version.to_ascii_lowercase().contains("redlinedb") {
        bail!(
            "{} version did not return a canonical RedlineDB identity: {version:?}",
            executable.display()
        );
    }
    Ok(version)
}

fn resolve_executable(path: &Path) -> Result<PathBuf> {
    if path.components().count() > 1 {
        return fs::canonicalize(path).with_context(|| format!("resolve {}", path.display()));
    }
    let name = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("executable name is not UTF-8"))?;
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join(name);
        if candidate.is_file() {
            return fs::canonicalize(&candidate)
                .with_context(|| format!("resolve {}", candidate.display()));
        }
    }
    bail!("executable not found: {}", path.display())
}

fn extract_semver(version: &str) -> Option<String> {
    version.split_whitespace().find_map(|token| {
        let value = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '.'
                && character != '-'
                && character != '+'
        });
        let semantic = value
            .strip_prefix('v')
            .or_else(|| value.strip_prefix('V'))
            .unwrap_or(value);
        let core = semantic.split(['-', '+']).next()?;
        let mut fields = core.split('.');
        let valid = (0..3).all(|_| {
            fields.next().is_some_and(|field| {
                !field.is_empty() && field.bytes().all(|byte| byte.is_ascii_digit())
            })
        }) && fields.next().is_none();
        valid.then(|| semantic.to_owned())
    })
}

fn corpus_hash(contract: &Contract) -> Result<String> {
    let root = runtime_data_root()?;
    let mut paths = Vec::new();
    collect_regular_files(&root.join(&contract.corpus_path), &mut paths)?;
    if let Some(exclusions) = &contract.exclusions_path {
        paths.push(root.join(exclusions));
    }
    paths.sort();
    let mut hasher = Sha256::new();
    for path in paths {
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(&path)?);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_regular_files(path: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        bail!(
            "compatibility corpus contains a symlink: {}",
            path.display()
        );
    }
    if metadata.is_file() {
        output.push(path.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        bail!(
            "compatibility corpus contains a special node: {}",
            path.display()
        );
    }
    for entry in fs::read_dir(path)? {
        collect_regular_files(&entry?.path(), output)?;
    }
    Ok(())
}

pub(crate) fn compatibility_evidence_path(output: &Path) -> Result<PathBuf> {
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("--output must have a UTF-8 filename"))?;
    Ok(output.with_file_name(format!("{name}.compat-evidence.json")))
}

pub(crate) fn select_postgres_oracle_cases(selector: &str) -> Result<Vec<BeyondCase>> {
    let all = beyond_sqlite::oracle::load_cases()?;
    validate_unique_ids(
        all.iter().map(|case| case.id),
        "PostgreSQL oracle compatibility corpus",
    )?;
    let contract = Contract {
        id: POSTGRES_CONTRACT.to_owned(),
        compatibility_mode: "postgres-oracle".to_owned(),
        oracle: "postgres".to_owned(),
        corpus_path: "corpus/beyond_sqlite/generated_manifest.json".to_owned(),
        exclusions_path: None,
        expected_case_count: all.len(),
        required_case_count: all.len(),
        exclusion_count: 0,
    };
    let descriptors = all.iter().map(postgres_descriptor).collect::<Vec<_>>();
    let selection = select_cases(selector, &descriptors, &contract)?;
    Ok(all
        .into_iter()
        .filter(|case| selection.ids.contains(&case.id))
        .collect())
}

pub(crate) fn postgres_exclusions_json() -> Result<Vec<serde_json::Value>> {
    let manifest = load_and_validate_manifest()?;
    let contract = manifest
        .contract
        .iter()
        .find(|contract| contract.id == POSTGRES_CONTRACT)
        .context("PostgreSQL contract is missing")?;
    let cases = beyond_sqlite::oracle::load_cases()?;
    load_and_validate_exclusions(&cases, contract)?;
    let exclusions: ExclusionManifest = toml::from_str(POSTGRES_EXCLUSIONS)?;
    Ok(exclusions
        .skip
        .into_iter()
        .map(|exclusion| serde_json::to_value(exclusion).expect("serialize exclusion"))
        .collect())
}

fn write_postgres_outcomes(path: &Path, outcomes: &[CaseOutcome]) -> Result<()> {
    let mut body = String::new();
    for outcome in outcomes {
        body.push_str(&serde_json::to_string(&json!({
            "schema_version": "redline.compat-case-result/v1",
            "case_id": outcome.case_id,
            "name": outcome.name,
            "category": outcome.category,
            "oracle_status": outcome.status,
            "oracle_diagnostic": outcome.diagnostic,
            "target_status": outcome.target.as_ref().map(|target| target.status.as_str()),
            "target_diagnostic": outcome.target.as_ref().and_then(|target| target.diagnostic.as_deref()),
        }))?);
        body.push('\n');
    }
    fs::write(path, body)?;
    Ok(())
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let mut body = serde_json::to_vec_pretty(value)?;
    body.push(b'\n');
    fs::write(path, body)?;
    Ok(())
}

fn run_mode(mode: RunMode) -> &'static str {
    match mode {
        RunMode::Diagnostic => "diagnostic",
        RunMode::Release => "release",
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn runtime_data_root() -> Result<PathBuf> {
    let Some(raw) = env::var_os("REDLINE_TESTING_RUNTIME_ROOT") else {
        return Ok(repo_root().to_path_buf());
    };
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        bail!("REDLINE_TESTING_RUNTIME_ROOT must be absolute");
    }
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("inspect REDLINE_TESTING_RUNTIME_ROOT at {}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!(
            "REDLINE_TESTING_RUNTIME_ROOT is not a physical directory: {}",
            path.display()
        );
    }
    fs::canonicalize(&path)
        .with_context(|| format!("resolve REDLINE_TESTING_RUNTIME_ROOT at {}", path.display()))
}

fn canonical_display(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root())
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub(crate) fn major_gate(baseline: &str, candidate: &str) -> Result<()> {
    let baseline_manifest = manifest_at_revision(baseline)?;
    let candidate_manifest = manifest_at_revision(candidate)?;
    compare_contracts(&baseline_manifest, &candidate_manifest)?;
    println!("redline compatibility major gate passed: baseline={baseline} candidate={candidate}");
    Ok(())
}

fn manifest_at_revision(revision: &str) -> Result<ContractManifest> {
    let object = format!("{revision}:{CONTRACT_MANIFEST_PATH}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root())
        .args(["show", &object])
        .output()?;
    if !output.status.success() {
        bail!(
            "cannot read compatibility contract at {revision}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    toml::from_str(&String::from_utf8(output.stdout)?)
        .context("parse compatibility manifest from Git")
}

fn compare_contracts(baseline: &ContractManifest, candidate: &ContractManifest) -> Result<()> {
    let candidates = candidate
        .contract
        .iter()
        .map(|contract| (contract.id.as_str(), contract))
        .collect::<BTreeMap<_, _>>();
    for previous in &baseline.contract {
        let next = candidates
            .get(previous.id.as_str())
            .ok_or_else(|| anyhow::anyhow!("candidate removed contract {}", previous.id))?;
        if next.required_case_count < previous.required_case_count {
            bail!(
                "candidate weakens {} required cases: {} -> {}",
                previous.id,
                previous.required_case_count,
                next.required_case_count
            );
        }
        if next.exclusion_count > previous.exclusion_count {
            bail!(
                "candidate increases {} exclusions: {} -> {}",
                previous.id,
                previous.exclusion_count,
                next.exclusion_count
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_contract_counts_match_the_embedded_corpora() {
        let manifest = load_and_validate_manifest().expect("contract manifest");
        let sqlite = manifest
            .contract
            .iter()
            .find(|contract| contract.id == SQLITE_CONTRACT)
            .unwrap();
        assert_eq!(
            sqlite_parity::all_cases().unwrap().len(),
            sqlite.required_case_count
        );

        let postgres = manifest
            .contract
            .iter()
            .find(|contract| contract.id == POSTGRES_CONTRACT)
            .unwrap();
        let cases = beyond_sqlite::oracle::load_cases().unwrap();
        let excluded = load_and_validate_exclusions(&cases, postgres).unwrap();
        assert_eq!(cases.len(), postgres.expected_case_count);
        assert_eq!(cases.len() - excluded.len(), postgres.required_case_count);
    }

    #[test]
    fn selector_is_deterministic_and_rejects_missing_cases() {
        let contract = Contract {
            id: SQLITE_CONTRACT.to_owned(),
            compatibility_mode: "sqlite".to_owned(),
            oracle: "sqlite".to_owned(),
            corpus_path: "corpus".to_owned(),
            exclusions_path: None,
            expected_case_count: 2,
            required_case_count: 2,
            exclusion_count: 0,
        };
        let cases = vec![
            CaseDescriptor {
                id: 2,
                category: "B".to_owned(),
                priority: "P1".to_owned(),
            },
            CaseDescriptor {
                id: 1,
                category: "A".to_owned(),
                priority: "P0".to_owned(),
            },
        ];
        let selected = select_cases("id:2,1", &cases, &contract).unwrap();
        assert_eq!(selected.ids.into_iter().collect::<Vec<_>>(), vec![1, 2]);
        assert!(select_cases("id:3", &cases, &contract).is_err());
    }

    #[test]
    fn major_gate_rejects_removed_or_weakened_contracts() {
        let baseline = load_and_validate_manifest().unwrap();
        let mut candidate = baseline.clone();
        candidate.contract[0].required_case_count -= 1;
        assert!(compare_contracts(&baseline, &candidate).is_err());
        let mut candidate = baseline.clone();
        candidate.contract.remove(0);
        assert!(compare_contracts(&baseline, &candidate).is_err());
    }

    #[test]
    fn semver_extraction_ignores_non_version_tokens() {
        assert_eq!(extract_semver("RedlineDB 4.2.0"), Some("4.2.0".to_owned()));
        assert_eq!(
            extract_semver("redlinedb v4.1.0 (SQLite 3.45.1 compatibility)"),
            Some("4.1.0".to_owned())
        );
        assert_eq!(extract_semver("redline-testing dev"), None);
    }
}
