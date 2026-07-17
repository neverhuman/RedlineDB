use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::fd::OwnedFd,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

use rustix::fs::{openat2, AtFlags, Mode, OFlags, ResolveFlags};

const AUTHORITY_FORMAT: &str = "jain.program-release-authority";
const EVIDENCE_FORMAT: &str = "jain.program-release-evidence-index";
const CUSTODY_FORMAT: &str = "jain.program-design-input-custody";
const STATUS_FORMAT: &str = "jain.program-release-status";
const MAX_AUTHORITY_BYTES: u64 = 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BASELINE_FILES: usize = 4096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramAuthority {
    format: String,
    program: Program,
    paths: ProgramPaths,
    firewall: WriterFirewall,
    canary: Canary,
    deployment_inputs: DeploymentInputs,
    design_inputs: DesignInputs,
    repository: Vec<ProgramRepository>,
    evidence_group: Vec<EvidencePolicy>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Program {
    name: String,
    release: String,
    claim: String,
    status: String,
    formal_ga: bool,
    activation_enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramPaths {
    design_input_root: String,
    spec: String,
    custody: String,
    evidence_root: String,
    evidence_index: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WriterFirewall {
    release_namespace: String,
    protected_releases: Vec<String>,
    protected_manifests: Vec<String>,
    protected_tag_fragments: Vec<String>,
    protected_surface: Vec<ProtectedSurface>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct ProtectedSurface {
    id: String,
    kind: BaselineKind,
    release: String,
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Canary {
    hub: String,
    nodes: Vec<String>,
    witnesses: Vec<String>,
    witness_quorum: usize,
    durability_claim: String,
    critical_available: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeploymentInputs {
    status: BindingStatus,
    required: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum BindingStatus {
    Unbound,
    Bound,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DesignInputs {
    source_root: String,
    names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramRepository {
    owner: String,
    name: String,
    checkout: String,
    remote: String,
    required_check: String,
    lifecycle: RepositoryLifecycle,
    head_commit: Option<String>,
    proof_receipt: Option<String>,
    proof_sha256: Option<String>,
    release_commit: Option<String>,
    release_tag: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum RepositoryLifecycle {
    NotCreated,
    LocalPrototype,
    ReviewPending,
    ProtectedMerged,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidencePolicy {
    id: String,
    policy: EvidencePolicyKind,
    validator: EvidenceValidator,
    requirements: Vec<String>,
    receipt_roots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EvidenceValidator {
    PortableCustody,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EvidencePolicyKind {
    Required,
    Deferred,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceIndex {
    format: String,
    release: String,
    authority_sha256: String,
    spec_sha256: String,
    groups: Vec<EvidenceGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceGroup {
    id: String,
    disposition: EvidenceDisposition,
    receipts: Vec<EvidenceReceipt>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EvidenceDisposition {
    Pending,
    Passed,
    Failed,
    Deferred,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvidenceReceipt {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustodyReceipt {
    format: String,
    release: String,
    source_root: String,
    live_source_checked: bool,
    intake_attested: bool,
    inputs: Vec<CustodyInput>,
    protected_baselines: Vec<ProtectedBaseline>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustodyInput {
    source: String,
    copy: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedBaseline {
    id: String,
    kind: BaselineKind,
    release: String,
    path: String,
    sha256: String,
    entries: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum BaselineKind {
    File,
    TreeInventory,
}

#[derive(Debug, Serialize)]
struct ReleaseStatus {
    format: &'static str,
    release: String,
    claim: String,
    decision: &'static str,
    eligible: bool,
    formal_ga: bool,
    activation_enabled: bool,
    critical_available: bool,
    authority_sha256: String,
    spec_sha256: String,
    custody_sha256: String,
    evidence_index_sha256: String,
    receipt_set_sha256: String,
    repository_set_sha256: String,
    decision_input_sha256: String,
    custody_live_source_checked: bool,
    custody_intake_attested: bool,
    passed: Vec<String>,
    pending: Vec<String>,
    failed: Vec<String>,
    deferred: Vec<String>,
    blockers: Vec<String>,
}

struct ValidatedProgram {
    root: PathBuf,
    authority: ProgramAuthority,
    index: EvidenceIndex,
    authority_sha256: String,
    spec_sha256: String,
    custody_sha256: String,
    evidence_index_sha256: String,
    receipt_set_sha256: String,
    repository_set_sha256: String,
    custody_live_source_checked: bool,
    custody_intake_attested: bool,
}

pub(crate) struct DeclaredCheckout {
    pub(crate) remote: Option<String>,
    pub(crate) must_be_absent: bool,
    pub(crate) expected_head: Option<String>,
}

pub(crate) fn declared_checkouts(
    control_root: &Path,
) -> Result<BTreeMap<PathBuf, DeclaredCheckout>, Box<dyn std::error::Error>> {
    let authority_paths = authority_paths(&control_root.join("authority"))?;
    let mut checkouts = BTreeMap::new();
    for authority_path in authority_paths.into_iter().filter(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".program-release.toml"))
    }) {
        let (canonical, bytes) = read_regular(&authority_path, MAX_AUTHORITY_BYTES)?;
        let authority: ProgramAuthority = toml::from_str(std::str::from_utf8(&bytes)?)?;
        validate_authority(control_root, &canonical, &authority)?;
        let family_root = control_root
            .parent()
            .ok_or("control-plane root has no family root")?;
        for repository in authority.repository {
            let path = family_root.join(&repository.name);
            let declared = match repository.lifecycle {
                RepositoryLifecycle::NotCreated => DeclaredCheckout {
                    remote: None,
                    must_be_absent: true,
                    expected_head: None,
                },
                RepositoryLifecycle::LocalPrototype => DeclaredCheckout {
                    remote: None,
                    must_be_absent: false,
                    expected_head: repository.head_commit,
                },
                RepositoryLifecycle::ReviewPending | RepositoryLifecycle::ProtectedMerged => {
                    DeclaredCheckout {
                        remote: Some(repository.remote),
                        must_be_absent: false,
                        expected_head: repository.release_commit.or(repository.head_commit),
                    }
                }
            };
            if checkouts.insert(path, declared).is_some() {
                return Err("program authorities declare the same checkout more than once".into());
            }
        }
    }
    Ok(checkouts)
}

pub fn command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(args)?;
    if args.operation == "validate-all" {
        if args.authority.is_some() || args.record.is_some() {
            return Err("program-release validate-all accepts only --authority-dir".into());
        }
        let directory = args
            .authority_dir
            .ok_or("program-release validate-all requires --authority-dir")?;
        return validate_all(&directory);
    }
    if args.authority_dir.is_some() {
        return Err("--authority-dir is accepted only by validate-all".into());
    }
    let authority = args.authority.ok_or("--authority is required")?;
    let program = validate_program(&authority)?;
    match args.operation.as_str() {
        "validate" => {
            if args.record.is_some() {
                return Err("program-release validate does not accept --record".into());
            }
            println!(
                "validated {} program authority and evidence index",
                program.authority.program.release
            );
        }
        "status" => {
            let status = reduce(&program);
            let bytes = serde_json::to_vec_pretty(&status)?;
            if let Some(record) = args.record {
                write_record(&program, &record, &bytes)?;
            } else {
                println!("{}", String::from_utf8(bytes)?);
            }
        }
        _ => {
            return Err(
                "program-release operation must be validate, validate-all, or status".into(),
            )
        }
    }
    Ok(())
}

struct CommandArgs {
    operation: String,
    authority: Option<PathBuf>,
    authority_dir: Option<PathBuf>,
    record: Option<PathBuf>,
}

fn parse_args(args: Vec<String>) -> Result<CommandArgs, Box<dyn std::error::Error>> {
    let mut iter = args.into_iter();
    let operation = iter
        .next()
        .ok_or("program-release requires validate, validate-all, or status")?;
    let mut authority = None;
    let mut authority_dir = None;
    let mut record = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--authority" => {
                authority = Some(PathBuf::from(
                    iter.next().ok_or("--authority needs a path")?,
                ))
            }
            "--authority-dir" => {
                authority_dir = Some(PathBuf::from(
                    iter.next().ok_or("--authority-dir needs a path")?,
                ))
            }
            "--record" => record = Some(PathBuf::from(iter.next().ok_or("--record needs a path")?)),
            value => return Err(format!("unknown program-release argument: {value}").into()),
        }
    }
    Ok(CommandArgs {
        operation,
        authority,
        authority_dir,
        record,
    })
}

fn authority_paths(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("program authority root must be a physical directory".into());
    }
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".program-release.toml"))
    });
    paths.sort();
    if paths.is_empty() {
        return Err("program authority root contains no program authorities".into());
    }
    for path in &paths {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("program authority discovery found a non-regular entry".into());
        }
    }
    Ok(paths)
}

fn validate_all(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let paths = authority_paths(directory)?;
    let mut releases = BTreeSet::new();
    let mut governed_roots = BTreeSet::new();
    let mut evidence_roots = BTreeSet::new();
    let mut checkouts = BTreeSet::new();
    for path in &paths {
        let program = validate_program(path)?;
        if !releases.insert(program.authority.program.release.clone())
            || !governed_roots.insert(program.authority.paths.design_input_root.clone())
            || !evidence_roots.insert(program.authority.paths.evidence_root.clone())
        {
            return Err("program authorities overlap a release or governed root".into());
        }
        for repository in &program.authority.repository {
            if !checkouts.insert(repository.checkout.clone()) {
                return Err("program authorities declare the same checkout more than once".into());
            }
        }
    }
    println!("validated {} program authorities", paths.len());
    Ok(())
}

fn validate_program(authority_path: &Path) -> Result<ValidatedProgram, Box<dyn std::error::Error>> {
    let (authority_path, authority_bytes) = read_regular(authority_path, MAX_AUTHORITY_BYTES)?;
    let authority_parent = authority_path
        .parent()
        .ok_or("program authority has no parent")?;
    if authority_parent
        .file_name()
        .and_then(|value| value.to_str())
        != Some("authority")
    {
        return Err("program authority must be stored beneath authority/".into());
    }
    let root = authority_parent
        .parent()
        .ok_or("program authority is not beneath a control-plane root")?
        .to_path_buf();
    let authority: ProgramAuthority = toml::from_str(std::str::from_utf8(&authority_bytes)?)?;
    validate_authority(&root, &authority_path, &authority)?;

    let expected_index = rooted_existing_file(
        &root,
        &authority.paths.evidence_index,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    let (_, index_bytes) = read_regular(&expected_index, MAX_INDEX_BYTES)?;
    let index: EvidenceIndex = serde_json::from_slice(&index_bytes)?;
    let spec_path = rooted_existing_file(
        &root,
        &authority.paths.spec,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    let (_, spec_bytes) = read_regular(&spec_path, MAX_RECEIPT_BYTES)?;
    validate_spec_requirements(&authority, &spec_bytes)?;
    let authority_sha256 = sha256(&authority_bytes);
    let spec_sha256 = sha256(&spec_bytes);
    validate_index(&root, &authority, &index, &authority_sha256, &spec_sha256)?;
    let custody = validate_custody(&root, &authority)?;
    let custody_path = rooted_existing_file(
        &root,
        &authority.paths.custody,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    let (_, custody_bytes) = read_regular(&custody_path, MAX_INDEX_BYTES)?;
    let custody_sha256 = sha256(&custody_bytes);
    let evidence_index_sha256 = sha256(&index_bytes);
    let receipt_set_sha256 = receipt_set_sha256(&index);
    let repository_set_sha256 = repository_set_sha256(&authority.repository);
    Ok(ValidatedProgram {
        root,
        authority,
        index,
        authority_sha256,
        spec_sha256,
        custody_sha256,
        evidence_index_sha256,
        receipt_set_sha256,
        repository_set_sha256,
        custody_live_source_checked: custody.live_source_checked,
        custody_intake_attested: custody.intake_attested,
    })
}

fn validate_authority(
    root: &Path,
    authority_path: &Path,
    authority: &ProgramAuthority,
) -> Result<(), Box<dyn std::error::Error>> {
    if authority.format != AUTHORITY_FORMAT {
        return Err("program authority format is unsupported".into());
    }
    validate_identifier("program.name", &authority.program.name)?;
    validate_release(&authority.program.release)?;
    validate_identifier("program.claim", &authority.program.claim)?;
    validate_identifier("program.status", &authority.program.status)?;
    if authority.program.formal_ga
        || authority.program.activation_enabled
        || authority.canary.critical_available
    {
        return Err(
            "GA, activation, and Critical claims require typed signed validators that are not yet available"
                .into(),
        );
    }
    if authority.firewall.release_namespace != authority.program.release
        || authority.firewall.protected_releases.is_empty()
        || authority.firewall.protected_manifests.is_empty()
        || authority.firewall.protected_tag_fragments.is_empty()
        || authority.firewall.protected_surface.is_empty()
        || authority
            .firewall
            .protected_releases
            .iter()
            .any(|release| release == &authority.program.release)
    {
        return Err("program writer firewall is incomplete or self-conflicting".into());
    }
    validate_unique_strings(
        "firewall.protected_releases",
        &authority.firewall.protected_releases,
    )?;
    validate_unique_strings(
        "firewall.protected_manifests",
        &authority.firewall.protected_manifests,
    )?;
    validate_unique_strings(
        "firewall.protected_tag_fragments",
        &authority.firewall.protected_tag_fragments,
    )?;
    validate_protected_surfaces(&authority.firewall)?;
    let authority_relative = authority_path.strip_prefix(root)?;
    for protected in &authority.firewall.protected_manifests {
        let protected = Path::new(protected);
        if protected.is_absolute()
            || protected
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("protected manifest paths must be normalized and relative".into());
        }
        if authority_relative == protected {
            return Err("program authority cannot alias a protected release manifest".into());
        }
    }
    for path in [
        &authority.paths.design_input_root,
        &authority.paths.spec,
        &authority.paths.custody,
        &authority.paths.evidence_root,
        &authority.paths.evidence_index,
    ] {
        validate_governed_path(
            Path::new(path),
            &authority.program.release,
            &authority.firewall.protected_releases,
        )?;
    }
    let design_root = Path::new(&authority.paths.design_input_root);
    if !Path::new(&authority.paths.spec).starts_with(design_root)
        || !Path::new(&authority.paths.custody).starts_with(design_root)
        || !Path::new(&authority.paths.evidence_index)
            .starts_with(Path::new(&authority.paths.evidence_root))
    {
        return Err("program spec, custody, or index escapes its declared governed root".into());
    }
    rooted_existing_directory(
        root,
        &authority.paths.design_input_root,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    rooted_existing_directory(
        root,
        &authority.paths.evidence_root,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    validate_canary(&authority.canary)?;
    validate_unique_strings(
        "deployment_inputs.required",
        &authority.deployment_inputs.required,
    )?;
    if authority.deployment_inputs.required.is_empty() {
        return Err("deployment input inventory must not be empty".into());
    }
    if authority.deployment_inputs.status == BindingStatus::Bound {
        return Err(
            "bound deployment inputs require a typed signed deployment validator that is not yet available"
                .into(),
        );
    }
    validate_design_inputs(&authority.design_inputs)?;
    validate_repositories(authority)?;
    validate_evidence_policies(authority)?;
    Ok(())
}

fn validate_canary(canary: &Canary) -> Result<(), Box<dyn std::error::Error>> {
    validate_identifier("canary.hub", &canary.hub)?;
    validate_identifier("canary.durability_claim", &canary.durability_claim)?;
    validate_unique_strings("canary.nodes", &canary.nodes)?;
    validate_unique_strings("canary.witnesses", &canary.witnesses)?;
    let nodes = canary
        .nodes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let witnesses = canary
        .witnesses
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if nodes.is_empty()
        || witnesses.is_empty()
        || !witnesses.is_subset(&nodes)
        || canary.witness_quorum == 0
        || canary.witness_quorum > witnesses.len()
        || canary.witness_quorum <= witnesses.len() / 2
    {
        return Err("canary nodes and witnesses do not form a strict majority quorum".into());
    }
    Ok(())
}

fn validate_protected_surfaces(
    firewall: &WriterFirewall,
) -> Result<(), Box<dyn std::error::Error>> {
    let protected_releases = firewall
        .protected_releases
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for surface in &firewall.protected_surface {
        validate_identifier("firewall.protected_surface.id", &surface.id)?;
        validate_release(&surface.release)?;
        if !ids.insert(surface.id.as_str())
            || !identities.insert((
                surface.kind,
                surface.release.as_str(),
                surface.path.as_str(),
            ))
            || !protected_releases.contains(surface.release.as_str())
        {
            return Err("protected surfaces contain a duplicate or unprotected release".into());
        }
        let path = Path::new(&surface.path);
        if path.is_absolute()
            || path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("protected surface paths must be normalized and relative".into());
        }
        let is_manifest = firewall
            .protected_manifests
            .iter()
            .any(|manifest| Path::new(manifest) == path);
        let contains_release = path
            .components()
            .any(|component| component.as_os_str() == surface.release.as_str());
        if !contains_release && !is_manifest {
            return Err("protected surface path does not bind its declared release".into());
        }
    }
    for release in &firewall.protected_releases {
        if !firewall
            .protected_surface
            .iter()
            .any(|surface| &surface.release == release)
        {
            return Err("every protected release needs an explicit protected surface".into());
        }
    }
    for manifest in &firewall.protected_manifests {
        let matches = firewall
            .protected_surface
            .iter()
            .filter(|surface| surface.kind == BaselineKind::File && surface.path == *manifest)
            .count();
        if matches != 1 {
            return Err("every protected manifest needs exactly one file surface".into());
        }
    }
    Ok(())
}

fn validate_design_inputs(inputs: &DesignInputs) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(
        Path::new(&inputs.source_root)
            .components()
            .collect::<Vec<_>>()
            .as_slice(),
        [Component::ParentDir]
    ) {
        return Err(
            "external design input provenance must name the containing family root exactly".into(),
        );
    }
    validate_unique_strings("design_inputs.names", &inputs.names)?;
    if inputs.names.is_empty() {
        return Err("design input inventory must not be empty".into());
    }
    for name in &inputs.names {
        let path = Path::new(name);
        if path.components().count() != 1
            || !matches!(path.components().next(), Some(Component::Normal(_)))
        {
            return Err("design input names must be single regular file names".into());
        }
    }
    Ok(())
}

fn validate_repositories(authority: &ProgramAuthority) -> Result<(), Box<dyn std::error::Error>> {
    if authority.repository.is_empty() {
        return Err("program authority must declare at least one repository".into());
    }
    let mut identities = BTreeSet::new();
    for repository in &authority.repository {
        validate_identifier("repository.owner", &repository.owner)?;
        validate_identifier("repository.name", &repository.name)?;
        if !identities.insert((repository.owner.as_str(), repository.name.as_str()))
            || repository.required_check != format!("{}/required", repository.name)
            || repository.remote
                != format!(
                    "http://127.0.0.1:8787/git/{}/{}.git",
                    repository.owner, repository.name
                )
            || !matches!(
                Path::new(&repository.checkout)
                    .components()
                    .collect::<Vec<_>>()
                    .as_slice(),
                [Component::ParentDir, Component::Normal(name)] if *name == repository.name.as_str()
            )
        {
            return Err("repository identity, remote, check, or checkout is invalid".into());
        }
        if let Some(commit) = &repository.head_commit {
            validate_hex("repository.head_commit", commit, 40)?;
        }
        if let Some(commit) = &repository.release_commit {
            validate_hex("repository.release_commit", commit, 40)?;
        }
        if let Some(path) = &repository.proof_receipt {
            validate_governed_path(
                Path::new(path),
                &authority.program.release,
                &authority.firewall.protected_releases,
            )?;
        }
        if let Some(digest) = &repository.proof_sha256 {
            validate_hex("repository.proof_sha256", digest, 64)?;
        }
        if let Some(tag) = &repository.release_tag {
            let prefix = format!("{}-v{}-", repository.name, authority.program.release);
            if !tag.starts_with(&prefix)
                || repository.release_commit.is_none()
                || authority
                    .firewall
                    .protected_releases
                    .iter()
                    .any(|release| tag.contains(release))
                || authority
                    .firewall
                    .protected_tag_fragments
                    .iter()
                    .any(|fragment| tag.contains(fragment))
            {
                return Err("release tag is legacy, malformed, or not commit-bound".into());
            }
        }
        let has_proof = repository.proof_receipt.is_some() && repository.proof_sha256.is_some();
        let proof_partial = repository.proof_receipt.is_some() != repository.proof_sha256.is_some();
        if proof_partial {
            return Err("repository proof path and digest must be supplied together".into());
        }
        match repository.lifecycle {
            RepositoryLifecycle::NotCreated => {
                if repository.head_commit.is_some()
                    || has_proof
                    || repository.release_commit.is_some()
                    || repository.release_tag.is_some()
                {
                    return Err(
                        "not-created repository cannot claim a head, proof, or release".into(),
                    );
                }
            }
            RepositoryLifecycle::LocalPrototype => {
                if repository.head_commit.is_none()
                    || has_proof
                    || repository.release_commit.is_some()
                    || repository.release_tag.is_some()
                {
                    return Err("local prototype requires only an exact local head".into());
                }
            }
            RepositoryLifecycle::ReviewPending => {
                if repository.head_commit.is_none()
                    || !has_proof
                    || repository.release_commit.is_some()
                    || repository.release_tag.is_some()
                {
                    return Err(
                        "review-pending repository needs an exact head and proof only".into(),
                    );
                }
            }
            RepositoryLifecycle::ProtectedMerged => {
                if !has_proof
                    || repository.release_commit.is_none()
                    || repository.release_tag.is_none()
                    || repository.head_commit.as_ref() != repository.release_commit.as_ref()
                {
                    return Err(
                        "protected-merged repository needs one exact head, proof, commit, and tag"
                            .into(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_evidence_policies(
    authority: &ProgramAuthority,
) -> Result<(), Box<dyn std::error::Error>> {
    if authority.evidence_group.is_empty() {
        return Err("evidence policy must not be empty".into());
    }
    let mut ids = BTreeSet::new();
    for group in &authority.evidence_group {
        validate_identifier("evidence_group.id", &group.id)?;
        if !ids.insert(group.id.as_str()) {
            return Err("evidence policy contains a duplicate group".into());
        }
        validate_unique_strings("evidence_group.requirements", &group.requirements)?;
        if group.requirements.is_empty() {
            return Err("every evidence policy must route at least one requirement".into());
        }
        if group.policy == EvidencePolicyKind::Deferred {
            if !group.receipt_roots.is_empty() {
                return Err("deferred evidence policy cannot declare receipt roots".into());
            }
        } else if group.receipt_roots.is_empty() {
            return Err("required evidence policy needs at least one receipt root".into());
        }
        if group.validator == EvidenceValidator::PortableCustody && group.id != "custody" {
            return Err("portable custody validation is reserved for the custody group".into());
        }
        validate_unique_strings("evidence_group.receipt_roots", &group.receipt_roots)?;
        for root in &group.receipt_roots {
            validate_governed_path(
                Path::new(root),
                &authority.program.release,
                &authority.firewall.protected_releases,
            )?;
        }
    }
    Ok(())
}

fn validate_spec_requirements(
    authority: &ProgramAuthority,
    spec_bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = std::str::from_utf8(spec_bytes)?;
    let mut documented = BTreeSet::new();
    for line in spec.lines() {
        let trimmed = line.trim_start();
        let Some(candidate) = trimmed
            .strip_prefix("- `")
            .and_then(|value| value.split_once("`:").map(|(id, _)| id))
        else {
            continue;
        };
        validate_identifier("SPEC requirement", candidate)?;
        if !documented.insert(candidate) {
            return Err("SPEC contains a duplicate requirement ID".into());
        }
    }
    if documented.is_empty() {
        return Err("SPEC contains no stable requirement IDs".into());
    }
    let mut routed = BTreeSet::new();
    for group in &authority.evidence_group {
        for requirement in &group.requirements {
            if !routed.insert(requirement.as_str()) {
                return Err("a SPEC requirement is routed by more than one evidence group".into());
            }
        }
    }
    if documented != routed {
        return Err("evidence policy must route every SPEC requirement exactly once".into());
    }
    Ok(())
}

fn validate_index(
    _root: &Path,
    authority: &ProgramAuthority,
    index: &EvidenceIndex,
    authority_sha256: &str,
    spec_sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if index.format != EVIDENCE_FORMAT
        || index.release != authority.program.release
        || index.authority_sha256 != authority_sha256
        || index.spec_sha256 != spec_sha256
        || index.groups.len() != authority.evidence_group.len()
    {
        return Err("evidence index identity or group count is invalid".into());
    }
    validate_hex("evidence.authority_sha256", &index.authority_sha256, 64)?;
    validate_hex("evidence.spec_sha256", &index.spec_sha256, 64)?;
    let policies = authority
        .evidence_group
        .iter()
        .map(|policy| (policy.id.as_str(), policy))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for group in &index.groups {
        let policy = policies
            .get(group.id.as_str())
            .ok_or("evidence index contains an unknown group")?;
        if !seen.insert(group.id.as_str()) {
            return Err("evidence index contains a duplicate group".into());
        }
        match policy.policy {
            EvidencePolicyKind::Deferred => {
                if group.disposition != EvidenceDisposition::Deferred || !group.receipts.is_empty()
                {
                    return Err("out-of-cut evidence must remain explicit and receipt-free".into());
                }
            }
            EvidencePolicyKind::Required => {
                if group.disposition == EvidenceDisposition::Deferred {
                    return Err("required evidence cannot be deferred".into());
                }
                if group.disposition != EvidenceDisposition::Pending || !group.receipts.is_empty() {
                    return Err(format!(
                        "evidence group {} cannot leave pending until its typed signed validator is implemented",
                        group.id
                    )
                    .into());
                }
                let _ = policy.validator;
            }
        }
    }
    if seen.len() != policies.len() {
        return Err("evidence index omits a governed group".into());
    }
    Ok(())
}

fn validate_custody(
    root: &Path,
    authority: &ProgramAuthority,
) -> Result<CustodyReceipt, Box<dyn std::error::Error>> {
    let custody_path = rooted_existing_file(
        root,
        &authority.paths.custody,
        &authority.program.release,
        &authority.firewall.protected_releases,
    )?;
    let (_, bytes) = read_regular(&custody_path, MAX_INDEX_BYTES)?;
    let receipt: CustodyReceipt = serde_json::from_slice(&bytes)?;
    if receipt.format != CUSTODY_FORMAT
        || receipt.release != authority.program.release
        || receipt.source_root != authority.design_inputs.source_root
        || receipt.inputs.len() != authority.design_inputs.names.len()
        || receipt.protected_baselines.len() != authority.firewall.protected_surface.len()
    {
        return Err("design-input custody identity or inventory is invalid".into());
    }
    if receipt.live_source_checked || receipt.intake_attested {
        return Err(
            "portable custody cannot claim a live-source check or intake attestation without a typed signed receipt"
                .into(),
        );
    }
    let expected = authority
        .design_inputs
        .names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for input in &receipt.inputs {
        if !expected.contains(input.source.as_str()) || !seen.insert(input.source.as_str()) {
            return Err("custody receipt contains an unknown or duplicate design input".into());
        }
        validate_hex("custody.input.sha256", &input.sha256, 64)?;
        let expected_copy = Path::new(&authority.paths.design_input_root).join(&input.source);
        if Path::new(&input.copy) != expected_copy {
            return Err("custody copy path differs from the authority design-input root".into());
        }
        let copy_path = rooted_existing_file(
            root,
            &input.copy,
            &authority.program.release,
            &authority.firewall.protected_releases,
        )?;
        let (_, copy_bytes) = read_regular(&copy_path, MAX_RECEIPT_BYTES)?;
        if input.bytes != copy_bytes.len() as u64 || input.sha256 != sha256(&copy_bytes) {
            return Err(
                "custodied design input differs from its recorded external observation".into(),
            );
        }
    }
    let expected_baselines = authority
        .firewall
        .protected_surface
        .iter()
        .map(|surface| {
            (
                surface.id.as_str(),
                surface.kind,
                surface.release.as_str(),
                surface.path.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut actual_baselines = BTreeSet::new();
    for baseline in &receipt.protected_baselines {
        validate_identifier("custody.baseline.id", &baseline.id)?;
        validate_release(&baseline.release)?;
        if !authority
            .firewall
            .protected_releases
            .contains(&baseline.release)
        {
            return Err("custody baseline release is not protected by the writer firewall".into());
        }
        validate_hex("custody.baseline.sha256", &baseline.sha256, 64)?;
        if !actual_baselines.insert((
            baseline.id.as_str(),
            baseline.kind,
            baseline.release.as_str(),
            baseline.path.as_str(),
        )) {
            return Err("custody baseline contains a duplicate identity".into());
        }
        let baseline_path = rooted_protected_path(root, authority, baseline)?;
        match baseline.kind {
            BaselineKind::File => {
                let (_, baseline_bytes) = read_regular(&baseline_path, MAX_RECEIPT_BYTES)?;
                if baseline.entries != 1 || sha256(&baseline_bytes) != baseline.sha256 {
                    return Err("protected file baseline digest is invalid".into());
                }
            }
            BaselineKind::TreeInventory => {
                let (digest, entries) = tree_inventory(&baseline_path)?;
                if digest != baseline.sha256 || entries != baseline.entries {
                    return Err("protected tree baseline inventory is invalid".into());
                }
            }
        }
    }
    if actual_baselines != expected_baselines {
        return Err("custody baselines differ from the exact protected-surface set".into());
    }
    Ok(receipt)
}

fn reduce(program: &ValidatedProgram) -> ReleaseStatus {
    let mut passed = Vec::new();
    let mut pending = Vec::new();
    let mut failed = Vec::new();
    let mut deferred = Vec::new();
    for group in &program.index.groups {
        match group.disposition {
            EvidenceDisposition::Passed => passed.push(group.id.clone()),
            EvidenceDisposition::Pending => pending.push(group.id.clone()),
            EvidenceDisposition::Failed => failed.push(group.id.clone()),
            EvidenceDisposition::Deferred => deferred.push(group.id.clone()),
        }
    }
    passed.sort();
    pending.sort();
    failed.sort();
    deferred.sort();
    let mut blockers = failed
        .iter()
        .map(|group| format!("failed:{group}"))
        .chain(pending.iter().map(|group| format!("pending:{group}")))
        .collect::<Vec<_>>();
    if program.authority.deployment_inputs.status != BindingStatus::Bound {
        blockers.push("deployment_inputs:unbound".to_owned());
    }
    blockers.sort();
    let eligible = blockers.is_empty();
    let decision_input_sha256 = digest_fields(
        b"jain.program-release-decision-input\0",
        &[
            &program.authority_sha256,
            &program.spec_sha256,
            &program.custody_sha256,
            &program.evidence_index_sha256,
            &program.receipt_set_sha256,
            &program.repository_set_sha256,
        ],
    );
    ReleaseStatus {
        format: STATUS_FORMAT,
        release: program.authority.program.release.clone(),
        claim: program.authority.program.claim.clone(),
        decision: if eligible { "eligible" } else { "blocked" },
        eligible,
        formal_ga: program.authority.program.formal_ga,
        activation_enabled: program.authority.program.activation_enabled,
        critical_available: program.authority.canary.critical_available,
        authority_sha256: program.authority_sha256.clone(),
        spec_sha256: program.spec_sha256.clone(),
        custody_sha256: program.custody_sha256.clone(),
        evidence_index_sha256: program.evidence_index_sha256.clone(),
        receipt_set_sha256: program.receipt_set_sha256.clone(),
        repository_set_sha256: program.repository_set_sha256.clone(),
        decision_input_sha256,
        custody_live_source_checked: program.custody_live_source_checked,
        custody_intake_attested: program.custody_intake_attested,
        passed,
        pending,
        failed,
        deferred,
        blockers,
    }
}

struct ConfinedDir {
    file: File,
}

impl ConfinedDir {
    fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let lexical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let canonical = fs::canonicalize(&lexical)?;
        if canonical != lexical {
            return Err("confined root must be canonical and symlink-free".into());
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lexical)?;
        if !file.metadata()?.is_dir() {
            return Err("confined root is not a directory".into());
        }
        Ok(Self { file })
    }

    fn open_relative(
        &self,
        relative: &Path,
        flags: OFlags,
        mode: Mode,
    ) -> Result<File, Box<dyn std::error::Error>> {
        if relative.is_absolute()
            || relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("confined path must be nonempty, relative, and normalized".into());
        }
        let fd: OwnedFd = openat2(
            &self.file,
            relative,
            flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            mode,
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?;
        Ok(File::from(fd))
    }
}

fn write_record(
    program: &ValidatedProgram,
    record: &Path,
    bytes: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    if !record.is_absolute()
        || record
            .components()
            .any(|component| component == Component::ParentDir)
        || contains_protected_release(record, &program.authority.firewall.protected_releases)
    {
        return Err("release status output path is unsafe or targets a protected release".into());
    }
    let evidence_root = rooted_existing_directory(
        &program.root,
        &program.authority.paths.evidence_root,
        &program.authority.program.release,
        &program.authority.firewall.protected_releases,
    )?;
    let relative = record
        .strip_prefix(&evidence_root)
        .map_err(|_| "release status output must be beneath the governed evidence root")?;
    if relative.components().count() < 2
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("release status output must be beneath the governed evidence root".into());
    }
    let parent_relative = relative
        .parent()
        .ok_or("release status output has no parent")?;
    let name = relative
        .file_name()
        .ok_or("release status output has no file name")?;
    let evidence = ConfinedDir::open(&evidence_root)?;
    let parent = evidence.open_relative(
        parent_relative,
        OFlags::RDONLY | OFlags::DIRECTORY,
        Mode::empty(),
    )?;
    let parent_dir = ConfinedDir { file: parent };
    let mut file = parent_dir.open_relative(
        Path::new(name),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL,
        Mode::from_raw_mode(0o600),
    )?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.nlink() != 1 || opened.permissions().mode() & 0o777 != 0o600 {
        let _ = rustix::fs::unlinkat(&parent_dir.file, Path::new(name), AtFlags::empty());
        return Err("release status output identity or permissions are unsafe".into());
    }
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    parent_dir.file.sync_all()?;
    let reopened = evidence.open_relative(relative, OFlags::RDONLY, Mode::empty());
    let identity_matches = reopened.as_ref().is_ok_and(|reopened| {
        reopened.metadata().is_ok_and(|metadata| {
            metadata.dev() == opened.dev()
                && metadata.ino() == opened.ino()
                && metadata.nlink() == 1
        })
    });
    if !identity_matches {
        let _ = rustix::fs::unlinkat(&parent_dir.file, Path::new(name), AtFlags::empty());
        return Err("release status output parent changed during creation".into());
    }
    Ok(())
}

fn rooted_protected_path(
    root: &Path,
    authority: &ProgramAuthority,
    baseline: &ProtectedBaseline,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = Path::new(&baseline.path);
    if relative.is_absolute()
        || relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative
            .components()
            .any(|component| component.as_os_str() == authority.program.release.as_str())
    {
        return Err(
            "protected baseline path is absolute, unnormalized, or targets this release".into(),
        );
    }
    let is_manifest = authority
        .firewall
        .protected_manifests
        .iter()
        .any(|manifest| relative == Path::new(manifest));
    let contains_release = relative
        .components()
        .any(|component| component.as_os_str() == baseline.release.as_str());
    if !is_manifest && !contains_release {
        return Err("protected baseline path is outside its declared protected release".into());
    }
    let lexical = root.join(relative);
    ensure_no_symlink_components(root, &lexical)?;
    let confined = ConfinedDir::open(root)?;
    let opened = confined.open_relative(relative, OFlags::RDONLY, Mode::empty())?;
    let canonical = fs::canonicalize(&lexical)?;
    if canonical != lexical {
        return Err("protected baseline uses a symlink or non-canonical alias".into());
    }
    let path_metadata = fs::symlink_metadata(&lexical)?;
    let opened_metadata = opened.metadata()?;
    if path_metadata.dev() != opened_metadata.dev() || path_metadata.ino() != opened_metadata.ino()
    {
        return Err("protected baseline changed while it was opened".into());
    }
    Ok(canonical)
}

fn rooted_existing_file(
    root: &Path,
    relative: &str,
    release: &str,
    protected_releases: &[String],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = rooted_existing(root, relative, release, protected_releases)?;
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err("governed path is not a regular file".into());
    }
    Ok(path)
}

fn rooted_existing_directory(
    root: &Path,
    relative: &str,
    release: &str,
    protected_releases: &[String],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = rooted_existing(root, relative, release, protected_releases)?;
    if !fs::symlink_metadata(&path)?.is_dir() {
        return Err("governed path is not a directory".into());
    }
    Ok(path)
}

fn rooted_existing(
    root: &Path,
    relative: &str,
    release: &str,
    protected_releases: &[String],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = Path::new(relative);
    validate_governed_path(relative, release, protected_releases)?;
    let lexical = root.join(relative);
    ensure_no_symlink_components(root, &lexical)?;
    let confined = ConfinedDir::open(root)?;
    let opened = confined.open_relative(relative, OFlags::RDONLY, Mode::empty())?;
    let canonical = fs::canonicalize(&lexical)?;
    if canonical != lexical {
        return Err("governed path uses a symlink or non-canonical alias".into());
    }
    let path_metadata = fs::symlink_metadata(&lexical)?;
    let opened_metadata = opened.metadata()?;
    if path_metadata.dev() != opened_metadata.dev() || path_metadata.ino() != opened_metadata.ino()
    {
        return Err("governed path changed while it was opened".into());
    }
    Ok(canonical)
}

fn validate_governed_path(
    path: &Path,
    release: &str,
    protected_releases: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err("governed paths must be nonempty and relative".into());
    }
    let mut contains_release = false;
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err("governed paths must be lexically normalized".into());
        };
        let value = value.to_string_lossy();
        contains_release |= value == release;
        if protected_releases
            .iter()
            .any(|protected| value == *protected)
        {
            return Err("governed path targets a protected release".into());
        }
    }
    if !contains_release {
        return Err("governed path omits the authority release namespace".into());
    }
    Ok(())
}

fn ensure_no_symlink_components(
    root: &Path,
    target: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let relative = target.strip_prefix(root)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if fs::symlink_metadata(&current)?.file_type().is_symlink() {
            return Err("governed path contains a symlink".into());
        }
    }
    Ok(())
}

fn read_regular(
    path: &Path,
    max_bytes: u64,
) -> Result<(PathBuf, Vec<u8>), Box<dyn std::error::Error>> {
    let lexical = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if lexical
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err("input paths must be lexically normalized".into());
    }
    let metadata = fs::symlink_metadata(&lexical)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > max_bytes
    {
        return Err("input is not a bounded regular non-symlink file".into());
    }
    let canonical = fs::canonicalize(&lexical)?;
    if canonical != lexical {
        return Err("input path uses a symlink or non-canonical alias".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&canonical)?;
    let opened = file.metadata()?;
    if !opened.is_file()
        || opened.len() != metadata.len()
        || opened.dev() != metadata.dev()
        || opened.ino() != metadata.ino()
        || opened.nlink() != 1
        || opened.permissions().mode() & 0o002 != 0
    {
        return Err("input identity or permissions are unsafe".into());
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if after.len() != opened.len()
        || after.dev() != opened.dev()
        || after.ino() != opened.ino()
        || bytes.len() as u64 != opened.len()
    {
        return Err("input changed while it was read".into());
    }
    Ok((canonical, bytes))
}

fn tree_inventory(path: &Path) -> Result<(String, usize), Box<dyn std::error::Error>> {
    let root = ConfinedDir::open(path)?;
    let mut records = Vec::new();
    collect_tree_records(&root.file, Path::new(""), &mut records)?;
    records.sort();
    let references = records.iter().map(String::as_str).collect::<Vec<_>>();
    Ok((
        digest_fields(b"jain.program-protected-tree\0", &references),
        records.len(),
    ))
}

fn collect_tree_records(
    directory: &File,
    prefix: &Path,
    records: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries = rustix::fs::Dir::read_from(directory)?;
    for entry in &mut entries {
        let entry = entry?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|_| "tree baseline contains a non-UTF8 name")?;
        if matches!(name, "." | "..") {
            continue;
        }
        let relative = prefix.join(name);
        let child_fd: OwnedFd = openat2(
            directory,
            Path::new(name),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?;
        let mut child = File::from(child_fd);
        let metadata = child.metadata()?;
        if metadata.is_dir() {
            collect_tree_records(&child, &relative, records)?;
        } else if metadata.is_file() {
            if records.len() >= MAX_BASELINE_FILES {
                return Err("tree baseline exceeds its file-count bound".into());
            }
            if metadata.len() > MAX_RECEIPT_BYTES
                || metadata.nlink() != 1
                || metadata.permissions().mode() & 0o002 != 0
            {
                return Err("tree baseline contains an unsafe file".into());
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            child.read_to_end(&mut bytes)?;
            let after = child.metadata()?;
            if after.dev() != metadata.dev()
                || after.ino() != metadata.ino()
                || after.len() != metadata.len()
                || bytes.len() as u64 != metadata.len()
            {
                return Err("tree baseline file changed while it was read".into());
            }
            records.push(format!(
                "{}\0{:o}\0{}\0{}",
                relative.display(),
                metadata.permissions().mode() & 0o7777,
                metadata.len(),
                sha256(&bytes)
            ));
        } else {
            return Err("tree baseline contains a non-file entry".into());
        }
    }
    Ok(())
}

fn contains_protected_release(path: &Path, protected_releases: &[String]) -> bool {
    path.components().any(|component| {
        protected_releases
            .iter()
            .any(|release| component.as_os_str() == release.as_str())
    })
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut unique = BTreeSet::new();
    for value in values {
        if value.is_empty() || !unique.insert(value) {
            return Err(format!("{field} contains an empty or duplicate value").into());
        }
    }
    Ok(())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    if value.is_empty()
        || value.len() > 128
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!("{field} is not a bounded identifier").into());
    }
    Ok(())
}

fn validate_release(value: &str) -> Result<(), Box<dyn std::error::Error>> {
    validate_identifier("release", value)?;
    if !value.bytes().any(|byte| byte.is_ascii_digit()) {
        return Err("release identity must contain at least one digit".into());
    }
    Ok(())
}

fn validate_hex(field: &str, value: &str, length: usize) -> Result<(), Box<dyn std::error::Error>> {
    if value.len() != length
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{field} must be lowercase hexadecimal with length {length}").into());
    }
    Ok(())
}

fn receipt_set_sha256(index: &EvidenceIndex) -> String {
    let mut fields = index
        .groups
        .iter()
        .flat_map(|group| {
            group
                .receipts
                .iter()
                .map(move |receipt| format!("{}\0{}\0{}", group.id, receipt.path, receipt.sha256))
        })
        .collect::<Vec<_>>();
    fields.sort();
    let references = fields.iter().map(String::as_str).collect::<Vec<_>>();
    digest_fields(b"jain.program-release-receipt-set\0", &references)
}

fn repository_set_sha256(repositories: &[ProgramRepository]) -> String {
    let mut fields = repositories
        .iter()
        .map(|repository| {
            format!(
                "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
                repository.owner,
                repository.name,
                lifecycle_name(repository.lifecycle),
                repository.head_commit.as_deref().unwrap_or("-"),
                repository.proof_receipt.as_deref().unwrap_or("-"),
                repository.proof_sha256.as_deref().unwrap_or("-"),
                repository.release_commit.as_deref().unwrap_or("-"),
                repository.release_tag.as_deref().unwrap_or("-")
            )
        })
        .collect::<Vec<_>>();
    fields.sort();
    let references = fields.iter().map(String::as_str).collect::<Vec<_>>();
    digest_fields(b"jain.program-release-repository-set\0", &references)
}

fn lifecycle_name(lifecycle: RepositoryLifecycle) -> &'static str {
    match lifecycle {
        RepositoryLifecycle::NotCreated => "not-created",
        RepositoryLifecycle::LocalPrototype => "local-prototype",
        RepositoryLifecycle::ReviewPending => "review-pending",
        RepositoryLifecycle::ProtectedMerged => "protected-merged",
    }
}

fn digest_fields(domain: &[u8], fields: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked_authorities() -> Vec<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        authority_paths(&root.join("authority")).unwrap()
    }

    fn tracked_authority() -> PathBuf {
        tracked_authorities().into_iter().next().unwrap()
    }

    fn parsed_tracked_authority() -> (PathBuf, PathBuf, ProgramAuthority) {
        let path = tracked_authority();
        let (path, bytes) = read_regular(&path, MAX_AUTHORITY_BYTES).unwrap();
        let root = path.parent().unwrap().parent().unwrap().to_path_buf();
        let authority = toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
        (root, path, authority)
    }

    #[test]
    fn tracked_program_authority_and_custody_validate() {
        for authority in tracked_authorities() {
            let program = validate_program(&authority).unwrap();
            assert!(!program.authority.program.release.is_empty());
            assert_eq!(
                program.index.groups.len(),
                program.authority.evidence_group.len()
            );
        }
    }

    #[test]
    fn protected_manifests_cannot_be_used_as_program_authority() {
        let program = validate_program(&tracked_authority()).unwrap();
        for protected in &program.authority.firewall.protected_manifests {
            assert!(validate_program(&program.root.join(protected)).is_err());
        }
    }

    #[test]
    fn omitted_and_falsely_promoted_evidence_fail_closed() {
        let program = validate_program(&tracked_authority()).unwrap();
        let authority_bytes = fs::read(tracked_authority()).unwrap();
        let spec_bytes = fs::read(program.root.join(&program.authority.paths.spec)).unwrap();

        let mut omitted = program.index.clone();
        omitted.groups.pop();
        assert!(validate_index(
            &program.root,
            &program.authority,
            &omitted,
            &sha256(&authority_bytes),
            &sha256(&spec_bytes),
        )
        .is_err());

        let mut promoted = program.index.clone();
        let deferred = promoted
            .groups
            .iter_mut()
            .find(|group| group.disposition == EvidenceDisposition::Deferred)
            .unwrap();
        deferred.disposition = EvidenceDisposition::Passed;
        assert!(validate_index(
            &program.root,
            &program.authority,
            &promoted,
            &sha256(&authority_bytes),
            &sha256(&spec_bytes),
        )
        .is_err());

        let mut opaque = program.index.clone();
        let required = opaque
            .groups
            .iter_mut()
            .find(|group| group.disposition == EvidenceDisposition::Pending)
            .unwrap();
        required.disposition = EvidenceDisposition::Passed;
        required.receipts.push(EvidenceReceipt {
            path: program.authority.paths.custody.clone(),
            sha256: program.custody_sha256.clone(),
        });
        assert!(validate_index(
            &program.root,
            &program.authority,
            &opaque,
            &sha256(&authority_bytes),
            &sha256(&spec_bytes),
        )
        .is_err());
    }

    #[test]
    fn writer_rejects_every_protected_release_component() {
        let program = validate_program(&tracked_authority()).unwrap();
        for protected in &program.authority.firewall.protected_releases {
            let target = PathBuf::from("/tmp")
                .join(protected)
                .join("release-status.json");
            assert!(write_record(&program, &target, b"{}").is_err());
        }
    }

    #[test]
    fn governed_paths_are_release_data_driven() {
        let protected = vec!["legacy-7".to_owned(), "stable-8".to_owned()];
        assert!(validate_governed_path(
            Path::new("evidence/candidate-12/receipt.json"),
            "candidate-12",
            &protected,
        )
        .is_ok());
        assert!(validate_governed_path(
            Path::new("evidence/stable-8/receipt.json"),
            "candidate-12",
            &protected,
        )
        .is_err());
        assert!(validate_governed_path(
            Path::new("evidence/candidate-12/../stable-8/receipt.json"),
            "candidate-12",
            &protected,
        )
        .is_err());
    }

    #[test]
    fn governed_paths_reject_lexical_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = std::env::temp_dir().join(format!(
            "jain-program-release-symlink-test-{}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let root = fs::canonicalize(&directory).unwrap();
        fs::create_dir(root.join("candidate-12")).unwrap();
        fs::write(root.join("candidate-12/file"), b"proof").unwrap();
        symlink("candidate-12", root.join("alias")).unwrap();
        assert!(rooted_existing_file(&root, "candidate-12/file", "candidate-12", &[]).is_ok());
        assert!(rooted_existing_file(&root, "alias/file", "alias", &[]).is_err());
        fs::remove_file(root.join("alias")).unwrap();
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unproved_release_claims_are_rejected() {
        let (root, path, mut authority) = parsed_tracked_authority();
        authority.program.formal_ga = true;
        assert!(validate_authority(&root, &path, &authority).is_err());

        let (root, path, mut authority) = parsed_tracked_authority();
        authority.program.activation_enabled = true;
        assert!(validate_authority(&root, &path, &authority).is_err());

        let (root, path, mut authority) = parsed_tracked_authority();
        authority.canary.critical_available = true;
        assert!(validate_authority(&root, &path, &authority).is_err());

        let (root, path, mut authority) = parsed_tracked_authority();
        authority.deployment_inputs.status = BindingStatus::Bound;
        assert!(validate_authority(&root, &path, &authority).is_err());
    }

    #[test]
    fn reduced_status_is_deterministic_and_input_bound() {
        let mut program = validate_program(&tracked_authority()).unwrap();
        let first = reduce(&program);
        let second = reduce(&program);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        let first_digest = first.decision_input_sha256;
        program.evidence_index_sha256 = "00".repeat(32);
        assert_ne!(first_digest, reduce(&program).decision_input_sha256);
    }

    #[test]
    fn witness_quorum_requires_a_strict_majority() {
        let valid = Canary {
            hub: "hub-a".to_owned(),
            nodes: vec![
                "node-a".to_owned(),
                "node-b".to_owned(),
                "node-c".to_owned(),
            ],
            witnesses: vec![
                "node-a".to_owned(),
                "node-b".to_owned(),
                "node-c".to_owned(),
            ],
            witness_quorum: 2,
            durability_claim: "durable".to_owned(),
            critical_available: false,
        };
        assert!(validate_canary(&valid).is_ok());
        let invalid = Canary {
            witness_quorum: 1,
            ..valid
        };
        assert!(validate_canary(&invalid).is_err());
    }

    #[test]
    fn evidence_schema_rejects_unknown_fields() {
        let invalid = r#"{
            "format":"jain.program-release-evidence-index",
            "release":"candidate-12",
            "authority_sha256":"00",
            "spec_sha256":"00",
            "groups":[],
            "passed":true
        }"#;
        assert!(serde_json::from_str::<EvidenceIndex>(invalid).is_err());
    }
}
