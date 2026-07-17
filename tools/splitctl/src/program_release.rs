use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

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
    receipt_roots: Vec<String>,
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
    kind: BaselineKind,
    release: String,
    path: String,
    sha256: String,
    entries: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
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
}

pub(crate) struct DeclaredCheckout {
    pub(crate) remote: Option<String>,
    pub(crate) must_be_absent: bool,
}

pub(crate) fn declared_checkouts(
    control_root: &Path,
) -> Result<BTreeMap<PathBuf, DeclaredCheckout>, Box<dyn std::error::Error>> {
    let authority_root = control_root.join("authority");
    let mut authority_paths = fs::read_dir(&authority_root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    authority_paths.sort();
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
                },
                RepositoryLifecycle::LocalPrototype => DeclaredCheckout {
                    remote: None,
                    must_be_absent: false,
                },
                RepositoryLifecycle::ReviewPending | RepositoryLifecycle::ProtectedMerged => {
                    DeclaredCheckout {
                        remote: Some(repository.remote),
                        must_be_absent: false,
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
    let (operation, authority, record) = parse_args(args)?;
    let program = validate_program(&authority)?;
    match operation.as_str() {
        "validate" => {
            if record.is_some() {
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
            if let Some(record) = record {
                write_record(&program, &record, &bytes)?;
            } else {
                println!("{}", String::from_utf8(bytes)?);
            }
        }
        _ => return Err("program-release operation must be validate or status".into()),
    }
    Ok(())
}

fn parse_args(
    args: Vec<String>,
) -> Result<(String, PathBuf, Option<PathBuf>), Box<dyn std::error::Error>> {
    let mut iter = args.into_iter();
    let operation = iter
        .next()
        .ok_or("program-release requires validate or status")?;
    let mut authority = None;
    let mut record = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--authority" => {
                authority = Some(PathBuf::from(
                    iter.next().ok_or("--authority needs a path")?,
                ))
            }
            "--record" => record = Some(PathBuf::from(iter.next().ok_or("--record needs a path")?)),
            value => return Err(format!("unknown program-release argument: {value}").into()),
        }
    }
    Ok((
        operation,
        authority.ok_or("--authority is required")?,
        record,
    ))
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
    validate_index(
        &root,
        &authority,
        &index,
        &sha256(&authority_bytes),
        &sha256(&spec_bytes),
    )?;
    validate_custody(&root, &authority)?;
    Ok(ValidatedProgram {
        root,
        authority,
        index,
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
    if authority.firewall.release_namespace != authority.program.release
        || authority.firewall.protected_releases.is_empty()
        || authority.firewall.protected_manifests.is_empty()
        || authority.firewall.protected_tag_fragments.is_empty()
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

fn validate_design_inputs(inputs: &DesignInputs) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(&inputs.source_root).is_absolute() || inputs.source_root.is_empty() {
        return Err("design input source root must be relative".into());
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
        if let Some(commit) = &repository.release_commit {
            validate_hex("repository.release_commit", commit, 40)?;
            if repository.lifecycle != RepositoryLifecycle::ProtectedMerged {
                return Err("a release commit requires protected-merged lifecycle".into());
            }
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
        if group.policy == EvidencePolicyKind::Deferred {
            if !group.receipt_roots.is_empty() {
                return Err("deferred evidence policy cannot declare receipt roots".into());
            }
        } else if group.receipt_roots.is_empty() {
            return Err("required evidence policy needs at least one receipt root".into());
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

fn validate_index(
    root: &Path,
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
                match group.disposition {
                    EvidenceDisposition::Pending if !group.receipts.is_empty() => {
                        return Err("pending evidence cannot claim receipts".into())
                    }
                    EvidenceDisposition::Passed | EvidenceDisposition::Failed
                        if group.receipts.is_empty() =>
                    {
                        return Err("passed or failed evidence requires a receipt".into())
                    }
                    _ => {}
                }
                for receipt in &group.receipts {
                    validate_hex("evidence.receipt.sha256", &receipt.sha256, 64)?;
                    validate_governed_path(
                        Path::new(&receipt.path),
                        &authority.program.release,
                        &authority.firewall.protected_releases,
                    )?;
                    if !policy
                        .receipt_roots
                        .iter()
                        .any(|allowed| Path::new(&receipt.path).starts_with(allowed))
                    {
                        return Err("evidence receipt is outside its governed roots".into());
                    }
                    let receipt_path = rooted_existing_file(
                        root,
                        &receipt.path,
                        &authority.program.release,
                        &authority.firewall.protected_releases,
                    )?;
                    let (_, bytes) = read_regular(&receipt_path, MAX_RECEIPT_BYTES)?;
                    if sha256(&bytes) != receipt.sha256 {
                        return Err("evidence receipt digest does not match its bytes".into());
                    }
                }
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
) -> Result<(), Box<dyn std::error::Error>> {
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
        || receipt.protected_baselines.is_empty()
    {
        return Err("design-input custody identity or inventory is invalid".into());
    }
    let source_root = resolve_source_root(root, &receipt.source_root)?;
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
        let (_, source_bytes) = read_regular(&source_root.join(&input.source), MAX_RECEIPT_BYTES)?;
        let copy_path = rooted_existing_file(
            root,
            &input.copy,
            &authority.program.release,
            &authority.firewall.protected_releases,
        )?;
        let (_, copy_bytes) = read_regular(&copy_path, MAX_RECEIPT_BYTES)?;
        if source_bytes != copy_bytes
            || input.bytes != source_bytes.len() as u64
            || input.sha256 != sha256(&source_bytes)
        {
            return Err("custodied design input is not byte-identical to its source".into());
        }
    }
    let mut baseline_paths = BTreeSet::new();
    for baseline in &receipt.protected_baselines {
        validate_release(&baseline.release)?;
        if !authority
            .firewall
            .protected_releases
            .contains(&baseline.release)
        {
            return Err("custody baseline release is not protected by the writer firewall".into());
        }
        validate_hex("custody.baseline.sha256", &baseline.sha256, 64)?;
        if !baseline_paths.insert(baseline.path.as_str()) {
            return Err("custody baseline contains a duplicate path".into());
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
    Ok(())
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
    ReleaseStatus {
        format: STATUS_FORMAT,
        release: program.authority.program.release.clone(),
        claim: program.authority.program.claim.clone(),
        decision: if eligible { "eligible" } else { "blocked" },
        eligible,
        formal_ga: program.authority.program.formal_ga,
        activation_enabled: program.authority.program.activation_enabled,
        critical_available: program.authority.canary.critical_available,
        passed,
        pending,
        failed,
        deferred,
        blockers,
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
    let parent = record
        .parent()
        .ok_or("release status output has no parent")?;
    let canonical_parent = fs::canonicalize(parent)?;
    if parent != canonical_parent
        || canonical_parent == evidence_root
        || !canonical_parent.starts_with(&evidence_root)
    {
        return Err("release status output must be beneath the governed evidence root".into());
    }
    ensure_no_symlink_components(&evidence_root, &canonical_parent)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(record)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(&canonical_parent)?.sync_all()?;
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
    let canonical = fs::canonicalize(root.join(relative))?;
    if !canonical.starts_with(root) {
        return Err("protected baseline escapes the control-plane root".into());
    }
    ensure_no_symlink_components(root, &canonical)?;
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
    let canonical = fs::canonicalize(root.join(relative))?;
    if !canonical.starts_with(root) {
        return Err("governed path escapes the control-plane root".into());
    }
    ensure_no_symlink_components(root, &canonical)?;
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

fn resolve_source_root(root: &Path, relative: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err("design input source root must be relative".into());
    }
    let canonical = fs::canonicalize(root.join(relative))?;
    if canonical
        != root
            .parent()
            .ok_or("control-plane root has no family root")?
    {
        return Err("design inputs must come from the containing family root".into());
    }
    if fs::symlink_metadata(&canonical)?.file_type().is_symlink() {
        return Err("design input source root cannot be a symlink".into());
    }
    Ok(canonical)
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
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&canonical)?;
    let opened = file.metadata()?;
    if !opened.is_file()
        || opened.len() != metadata.len()
        || opened.dev() != metadata.dev()
        || opened.ino() != metadata.ino()
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
    let canonical = fs::canonicalize(path)?;
    if !fs::symlink_metadata(&canonical)?.is_dir() {
        return Err("tree baseline is not a directory".into());
    }
    let mut files = Vec::new();
    collect_tree_files(&canonical, &canonical, &mut files)?;
    files.sort();
    let mut inventory = Vec::new();
    for relative in &files {
        let (_, bytes) = read_regular(&canonical.join(relative), MAX_RECEIPT_BYTES)?;
        writeln!(&mut inventory, "{}  {}", sha256(&bytes), relative.display())?;
    }
    Ok((sha256(&inventory), files.len()))
}

fn collect_tree_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err("tree baseline contains a symlink".into());
        }
        if metadata.is_dir() {
            collect_tree_files(root, &path, files)?;
        } else if metadata.is_file() {
            if files.len() >= MAX_BASELINE_FILES {
                return Err("tree baseline exceeds its file-count bound".into());
            }
            files.push(path.strip_prefix(root)?.to_path_buf());
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

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked_authority() -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut authorities = fs::read_dir(root.join("authority"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".program-release.toml"))
            })
            .collect::<Vec<_>>();
        authorities.sort();
        assert_eq!(
            authorities.len(),
            1,
            "test requires one governed program authority"
        );
        authorities.pop().unwrap()
    }

    #[test]
    fn tracked_program_authority_and_custody_validate() {
        let program = validate_program(&tracked_authority()).unwrap();
        assert!(!program.authority.program.release.is_empty());
        assert_eq!(
            program.index.groups.len(),
            program.authority.evidence_group.len()
        );
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
