use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use super::case::{Case, Profile};
use super::memory::ProcessMemory;
use super::text::sanitize_identifier;

/// Canonical name of the SQLite reference CLI that the harness drives over a
/// subprocess (never an in-process DB driver). Centralized here, in the
/// subprocess engine module, so the rest of the crate refers to it by symbol.
pub const REFERENCE_CLI_BIN: &str = "sqlite3";

#[derive(Debug, Clone)]
pub struct EngineSpec {
    pub name: String,
    pub bin: PathBuf,
    identity: Arc<OnceLock<Result<BinaryIdentity, String>>>,
}

#[derive(Debug, Clone)]
pub struct BinaryIdentity {
    pub executable_path: String,
    pub executable_sha256: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub engine: String,
    pub executable_path: String,
    pub executable_sha256: String,
    pub version: String,
    pub status_code: Option<i32>,
    pub elapsed: Duration,
    pub stdout: String,
    pub stderr: String,
    pub memory_status: String,
    pub peak_rss_kb: Option<u64>,
    pub rss_sampled_kb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    PercentileFunctions,
    DotCrlf,
    DotDbInfo,
    DotDbTotxt,
    DotRecover,
    EscapeSymbolOption,
    Fts5,
    Rtree,
    Dbstat,
    Jsonb,
    Math1,
    GenerateSeries,
    JsonPretty,
    JsonbArrayInsert,
}

impl Capability {
    pub fn description(self) -> &'static str {
        match self {
            Self::PercentileFunctions => "median()/percentile_cont()",
            Self::DotCrlf => ".crlf",
            Self::DotDbInfo => ".dbinfo",
            Self::DotDbTotxt => ".dbtotxt",
            Self::DotRecover => ".recover",
            Self::EscapeSymbolOption => "-escape symbol",
            Self::Fts5 => "fts5 virtual table",
            Self::Rtree => "rtree virtual table",
            Self::Dbstat => "dbstat virtual table",
            Self::Jsonb => "jsonb() (3.45+)",
            Self::Math1 => "math1 functions (acos/asin/sqrt/etc.)",
            Self::GenerateSeries => "generate_series virtual table",
            Self::JsonPretty => "json_pretty() (3.46+)",
            Self::JsonbArrayInsert => "jsonb_array_insert() (3.47+)",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "percentile_functions" => Some(Self::PercentileFunctions),
            "dot_crlf" => Some(Self::DotCrlf),
            "dot_dbinfo" => Some(Self::DotDbInfo),
            "dot_dbtotxt" => Some(Self::DotDbTotxt),
            "dot_recover" => Some(Self::DotRecover),
            "escape_symbol_option" => Some(Self::EscapeSymbolOption),
            "fts5" => Some(Self::Fts5),
            "rtree" => Some(Self::Rtree),
            "dbstat" => Some(Self::Dbstat),
            "jsonb" => Some(Self::Jsonb),
            "math1" => Some(Self::Math1),
            "generate_series" => Some(Self::GenerateSeries),
            "json_pretty" => Some(Self::JsonPretty),
            "jsonb_array_insert" => Some(Self::JsonbArrayInsert),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellCapabilities {
    pub version: String,
    pub percentile_functions: bool,
    pub dot_crlf: bool,
    pub dot_dbinfo: bool,
    pub dot_dbtotxt: bool,
    pub dot_recover: bool,
    pub escape_symbol_option: bool,
    pub fts5: bool,
    pub rtree: bool,
    pub dbstat: bool,
    pub jsonb: bool,
    pub math1: bool,
    pub generate_series: bool,
    pub json_pretty: bool,
    pub jsonb_array_insert: bool,
}

impl ShellCapabilities {
    pub fn supports(&self, capability: Capability) -> bool {
        match capability {
            Capability::PercentileFunctions => self.percentile_functions,
            Capability::DotCrlf => self.dot_crlf,
            Capability::DotDbInfo => self.dot_dbinfo,
            Capability::DotDbTotxt => self.dot_dbtotxt,
            Capability::DotRecover => self.dot_recover,
            Capability::EscapeSymbolOption => self.escape_symbol_option,
            Capability::Fts5 => self.fts5,
            Capability::Rtree => self.rtree,
            Capability::Dbstat => self.dbstat,
            Capability::Jsonb => self.jsonb,
            Capability::Math1 => self.math1,
            Capability::GenerateSeries => self.generate_series,
            Capability::JsonPretty => self.json_pretty,
            Capability::JsonbArrayInsert => self.jsonb_array_insert,
        }
    }
}

#[derive(Debug)]
pub struct SkippedCase {
    pub case: Case,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct CasePartition {
    pub runnable: Vec<Case>,
    pub skipped: Vec<SkippedCase>,
}

pub fn partition_cases(
    cases: Vec<Case>,
    reference_caps: Option<&ShellCapabilities>,
    target_caps: Option<&ShellCapabilities>,
) -> CasePartition {
    let mut partition = CasePartition::default();
    for case in cases {
        let required = required_capabilities(&case);
        if required.is_empty() {
            partition.runnable.push(case);
            continue;
        }
        let skip_reason = required.iter().copied().find_map(|capability| {
            if let Some(caps) = reference_caps
                && !caps.supports(capability)
            {
                return Some(format!(
                    "{} lacks {}",
                    shell_version_prefix(caps),
                    capability.description()
                ));
            }
            if let Some(caps) = target_caps
                && !caps.supports(capability)
            {
                return Some(format!(
                    "{} lacks {}",
                    shell_version_prefix(caps),
                    capability.description()
                ));
            }
            None
        });
        match skip_reason {
            Some(reason) => partition.skipped.push(SkippedCase { case, reason }),
            None => partition.runnable.push(case),
        }
    }
    partition
}

/// Capabilities a case needs. Pulls from two sources, in order of priority:
///
///   1. Legacy hardcoded table for pinned-manifest cases that don't carry
///      capability tokens (ids 92, 134, 154, 155, 156, 222).
///   2. The `required_capabilities` field on the case itself, populated by
///      new shards under `corpus/sqlite_parity/cases/`. Unknown tokens are
///      silently dropped — the case simply runs without gating on a
///      capability we don't know about (forwards-compatible).
pub fn required_capabilities(case: &Case) -> Vec<Capability> {
    let mut caps = match case.id {
        92 => vec![Capability::PercentileFunctions],
        93 | 94 => vec![Capability::Fts5],
        95 => vec![Capability::Rtree],
        96 => vec![Capability::Dbstat],
        134 => vec![Capability::DotCrlf],
        154 => vec![Capability::DotDbInfo],
        155 => vec![Capability::DotDbTotxt],
        156 => vec![Capability::DotRecover],
        222 => vec![Capability::EscapeSymbolOption],
        _ => Vec::new(),
    };
    for token in &case.required_capabilities {
        if let Some(cap) = Capability::from_token(token) {
            if !caps.contains(&cap) {
                caps.push(cap);
            }
        }
    }
    caps
}

pub fn probe_sqlite_shell_capabilities(bin: &Path) -> Result<ShellCapabilities> {
    let version = probe_version(bin)?;
    let memory_db = Path::new(":memory:");
    Ok(ShellCapabilities {
        version,
        percentile_functions: run_sql_script(
            bin,
            memory_db,
            ".mode list\n.headers off\nCREATE TABLE t(x INTEGER); INSERT INTO t VALUES (1), (2), (3);\nSELECT median(x), percentile_cont(x,0.5) FROM t;\n",
            &[],
        )?,
        dot_crlf: shell_help_contains(bin, ".crlf")?,
        dot_dbinfo: shell_help_contains(bin, ".dbinfo")?,
        dot_dbtotxt: shell_help_contains(bin, ".dbtotxt")?,
        dot_recover: shell_help_contains(bin, ".recover")?,
        escape_symbol_option: escape_symbol_option_supported(bin)?,
        fts5: run_sql_script(
            bin,
            memory_db,
            "CREATE VIRTUAL TABLE _probe_fts USING fts5(x);\n",
            &[],
        )?,
        rtree: run_sql_script(
            bin,
            memory_db,
            "CREATE VIRTUAL TABLE _probe_rtree USING rtree(id, x0, x1, y0, y1);\n",
            &[],
        )?,
        dbstat: run_sql_script(
            bin,
            memory_db,
            "CREATE TABLE _probe_t(x);\nINSERT INTO _probe_t VALUES(1);\nCREATE VIRTUAL TABLE main._probe_stat USING dbstat;\nSELECT count(*) FROM _probe_stat;\n",
            &[],
        )?,
        jsonb: run_sql_script(bin, memory_db, "SELECT length(jsonb('1'));\n", &[])?,
        math1: run_sql_script(
            bin,
            memory_db,
            "SELECT round(acos(1.0),3), round(sqrt(4.0),3);\n",
            &[],
        )?,
        generate_series: run_sql_script(
            bin,
            memory_db,
            "SELECT count(*) FROM generate_series(1,3);\n",
            &[],
        )?,
        json_pretty: run_sql_script(bin, memory_db, "SELECT json_pretty('{\"a\":1}');\n", &[])?,
        jsonb_array_insert: run_sql_script(
            bin,
            memory_db,
            "SELECT length(jsonb_array_insert('[]', '$[0]', 1));\n",
            &[],
        )?,
    })
}

/// Probe an arbitrary sqlite-shell-compatible binary (including the
/// `redlinedb-cli` target binary) for the SQL/CLI capabilities the
/// pinned-manifest cases gate on. We only probe the subset that
/// pinned-manifest cases actually require — bench-only or CLI-only
/// capabilities that no required-capability case names are skipped to
/// keep target startup cost down.
pub fn probe_target_capabilities(bin: &Path) -> Result<ShellCapabilities> {
    let version = probe_version(bin)?;
    let memory_db = Path::new(":memory:");
    let probe = |script: &str| run_sql_script(bin, memory_db, script, &[]);
    Ok(ShellCapabilities {
        version,
        // CLI-only / non-SQL capabilities — gate via shell help when
        // available, default to absent for engines that don't ship the
        // sqlite3 CLI help surface.
        percentile_functions: probe(
            ".mode list\n.headers off\nCREATE TABLE t(x INTEGER); INSERT INTO t VALUES (1), (2), (3);\nSELECT median(x), percentile_cont(x,0.5) FROM t;\n",
        )
        .unwrap_or(false),
        dot_crlf: shell_help_contains(bin, ".crlf").unwrap_or(false),
        dot_dbinfo: shell_help_contains(bin, ".dbinfo").unwrap_or(false),
        dot_dbtotxt: shell_help_contains(bin, ".dbtotxt").unwrap_or(false),
        dot_recover: shell_help_contains(bin, ".recover").unwrap_or(false),
        escape_symbol_option: escape_symbol_option_supported(bin).unwrap_or(false),
        fts5: probe("CREATE VIRTUAL TABLE _probe_fts USING fts5(x);\n").unwrap_or(false),
        rtree: probe("CREATE VIRTUAL TABLE _probe_rtree USING rtree(id, x0, x1, y0, y1);\n")
            .unwrap_or(false),
        dbstat: probe(
            "CREATE TABLE _probe_t(x);\nINSERT INTO _probe_t VALUES(1);\nCREATE VIRTUAL TABLE main._probe_stat USING dbstat;\nSELECT count(*) FROM _probe_stat;\n",
        )
        .unwrap_or(false),
        jsonb: probe("SELECT length(jsonb('1'));\n").unwrap_or(false),
        math1: probe("SELECT round(acos(1.0),3), round(sqrt(4.0),3);\n").unwrap_or(false),
        generate_series: probe("SELECT count(*) FROM generate_series(1,3);\n").unwrap_or(false),
        json_pretty: probe("SELECT json_pretty('{\"a\":1}');\n").unwrap_or(false),
        jsonb_array_insert: probe("SELECT length(jsonb_array_insert('[]', '$[0]', 1));\n")
            .unwrap_or(false),
    })
}

impl EngineSpec {
    pub fn new(name: impl Into<String>, bin: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            bin: bin.into(),
            identity: Arc::new(OnceLock::new()),
        }
    }

    pub fn run_case(
        &self,
        case: &Case,
        tmp_root: &Path,
        memory_samples: bool,
    ) -> Result<EngineOutput> {
        let identity = self.binary_identity()?;
        let case_tmp = tmp_root.join(format!(
            "{}-{}-{}",
            case.display_id(),
            sanitize_identifier(&self.name),
            std::process::id()
        ));
        if case_tmp.exists() {
            make_removable(&case_tmp).with_context(|| {
                format!(
                    "prepare previous sqlite parity tmpdir {} for removal",
                    case_tmp.display()
                )
            })?;
            fs::remove_dir_all(&case_tmp).with_context(|| {
                format!(
                    "remove previous sqlite parity tmpdir {}",
                    case_tmp.display()
                )
            })?;
        }
        fs::create_dir_all(&case_tmp)
            .with_context(|| format!("create sqlite parity tmpdir {}", case_tmp.display()))?;
        for (name, contents) in &case.files {
            let path = case_tmp.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create fixture parent {}", parent.display()))?;
            }
            fs::write(&path, replace_tmp(contents, &case_tmp))
                .with_context(|| format!("write fixture {}", path.display()))?;
        }

        let start = Instant::now();
        if let Some(script) = &case.script {
            return self.run_script(case, script, &case_tmp, start, identity, memory_samples);
        }

        let db_path = db_path_for(&self.name, case, tmp_root, &case_tmp)?;
        let mut command = Command::new(&self.bin);
        if case.args.is_empty() {
            if is_sqlite_shell(&self.name) {
                command.arg("-batch").arg("-bail").arg(&db_path);
            } else {
                command.arg("--batch").arg("--bail").arg(&db_path);
            }
        } else {
            for arg in &case.args {
                command.arg(replace_tmp(arg, &case_tmp));
            }
        }
        let output = run_command(
            &mut command,
            Some(replace_tmp(&case.stdin, &case_tmp)),
            &case_tmp,
            &self.name,
            memory_samples,
        )
        .with_context(|| format!("run {} case {}", self.name, case.display_id()))?;
        let elapsed = start.elapsed();
        Ok(EngineOutput {
            engine: self.name.clone(),
            executable_path: identity.executable_path,
            executable_sha256: identity.executable_sha256,
            version: identity.version,
            status_code: output.status.code(),
            elapsed,
            stdout: output.stdout,
            stderr: output.stderr,
            memory_status: output.memory.status(memory_samples).to_owned(),
            peak_rss_kb: output.memory.peak_rss_kb,
            rss_sampled_kb: output.memory.rss_sampled_kb,
        })
    }

    pub fn sqlite_shell_capabilities(&self) -> Result<Option<ShellCapabilities>> {
        if is_sqlite_shell(&self.name) {
            Ok(Some(probe_sqlite_shell_capabilities(&self.bin)?))
        } else {
            Ok(None)
        }
    }

    /// Probe the engine binary for the SQL/CLI capabilities required by
    /// pinned-manifest cases. Works for any sqlite-shell-compatible CLI
    /// (sqlite3 or redlinedb), so we can skip target-optional cases the
    /// target lacks without failing the parity gate.
    pub fn target_capabilities(&self) -> Result<ShellCapabilities> {
        probe_target_capabilities(&self.bin)
    }

    pub fn binary_identity(&self) -> Result<BinaryIdentity> {
        match self
            .identity
            .get_or_init(|| binary_identity(&self.bin).map_err(|err| err.to_string()))
        {
            Ok(identity) => Ok(identity.clone()),
            Err(message) => bail!("{message}"),
        }
    }

    fn run_script(
        &self,
        case: &Case,
        script: &str,
        case_tmp: &Path,
        start: Instant,
        identity: BinaryIdentity,
        memory_samples: bool,
    ) -> Result<EngineOutput> {
        let script_path = case_tmp.join("case.sh");
        fs::write(&script_path, replace_tmp(script, case_tmp))
            .with_context(|| format!("write script {}", script_path.display()))?;
        let mut command = Command::new("bash");
        command
            .arg(&script_path)
            .env("SQLITE_BIN", &self.bin)
            .env("SQLITE_PARITY_TMP", case_tmp);
        let output = run_command(&mut command, None, case_tmp, &self.name, memory_samples)
            .with_context(|| format!("run script case {}", case.display_id()))?;
        Ok(EngineOutput {
            engine: self.name.clone(),
            executable_path: identity.executable_path,
            executable_sha256: identity.executable_sha256,
            version: identity.version,
            status_code: output.status.code(),
            elapsed: start.elapsed(),
            stdout: output.stdout,
            stderr: output.stderr,
            memory_status: output.memory.status(memory_samples).to_owned(),
            peak_rss_kb: output.memory.peak_rss_kb,
            rss_sampled_kb: output.memory.rss_sampled_kb,
        })
    }
}

struct CapturedOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
    memory: ProcessMemory,
}

fn run_command(
    command: &mut Command,
    stdin_text: Option<String>,
    case_tmp: &Path,
    engine_name: &str,
    memory_samples: bool,
) -> Result<CapturedOutput> {
    if !memory_samples {
        command.stdin(if stdin_text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command.spawn().context("spawn sqlite parity child")?;
        if let Some(stdin_text) = stdin_text {
            let mut stdin = child
                .stdin
                .take()
                .context("child stdin unavailable for sqlite parity case")?;
            stdin
                .write_all(stdin_text.as_bytes())
                .context("write sqlite parity child stdin")?;
        }
        let output = child
            .wait_with_output()
            .context("wait sqlite parity child")?;
        return Ok(CapturedOutput {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            memory: ProcessMemory::default(),
        });
    }

    let output_prefix = sanitize_identifier(engine_name);
    let stdout_path = case_tmp.join(format!("{output_prefix}.stdout"));
    let stderr_path = case_tmp.join(format!("{output_prefix}.stderr"));
    command.stdin(if stdin_text.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    command.stdout(Stdio::from(
        fs::File::create(&stdout_path)
            .with_context(|| format!("create {}", stdout_path.display()))?,
    ));
    command.stderr(Stdio::from(
        fs::File::create(&stderr_path)
            .with_context(|| format!("create {}", stderr_path.display()))?,
    ));
    let mut child = command.spawn().context("spawn sqlite parity child")?;
    if let Some(stdin_text) = stdin_text {
        let mut stdin = child
            .stdin
            .take()
            .context("child stdin unavailable for sqlite parity case")?;
        stdin
            .write_all(stdin_text.as_bytes())
            .context("write sqlite parity child stdin")?;
    }
    let mut memory = ProcessMemory::default();
    let status = loop {
        if memory_samples {
            memory.observe_pid(child.id());
        }
        if let Some(status) = child.try_wait().context("poll sqlite parity child")? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    if memory_samples {
        memory.observe_pid(child.id());
    }
    Ok(CapturedOutput {
        status,
        stdout: read_output_lossy(&stdout_path)?,
        stderr: read_output_lossy(&stderr_path)?,
        memory,
    })
}

fn read_output_lossy(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn binary_identity(bin: &Path) -> Result<BinaryIdentity> {
    let path = resolve_executable_path(bin)?;
    let bytes = fs::read(&path).with_context(|| format!("read executable {}", path.display()))?;
    let executable_sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(BinaryIdentity {
        executable_path: path.to_string_lossy().into_owned(),
        executable_sha256,
        version: probe_version(&path)?,
    })
}

pub fn resolve_executable_path(path: &Path) -> Result<PathBuf> {
    if path.components().count() > 1 || path.is_absolute() {
        return fs::canonicalize(path)
            .with_context(|| format!("canonicalize executable {}", path.display()));
    }
    let Some(path_var) = std::env::var_os("PATH") else {
        bail!("PATH is unset while resolving {}", path.display());
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(path);
        if candidate.is_file() {
            return fs::canonicalize(&candidate)
                .with_context(|| format!("canonicalize executable {}", candidate.display()));
        }
    }
    bail!("executable not found on PATH: {}", path.display())
}

fn db_path_for(engine: &str, case: &Case, tmp_root: &Path, case_tmp: &Path) -> Result<String> {
    let db = replace_tmp(&case.db, case_tmp);
    if db != ":memory:" {
        return Ok(db);
    }
    match case.profile {
        Profile::Tempfile => {
            fs::create_dir_all(tmp_root)
                .with_context(|| format!("create sqlite parity tmpdir {}", tmp_root.display()))?;
            let path = case_tmp.join(format!("{}.db", sanitize_identifier(engine)));
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("non-utf8 sqlite parity db path {}", path.display()))
        }
        _ => Ok(":memory:".to_owned()),
    }
}

pub(crate) fn is_sqlite_shell(engine_name: &str) -> bool {
    engine_name.eq_ignore_ascii_case("sqlite3") || engine_name.eq_ignore_ascii_case("sqlite")
}

fn replace_tmp(input: &str, tmp: &Path) -> String {
    input.replace("{{TMP}}", &tmp.to_string_lossy())
}

fn probe_version(bin: &Path) -> Result<String> {
    let output = Command::new(bin)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run {} --version", bin.display()))?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        Ok(String::from("<unknown>"))
    } else {
        Ok(version)
    }
}

fn run_sql_script(bin: &Path, db_path: &Path, script: &str, extra_args: &[&str]) -> Result<bool> {
    let mut command = Command::new(bin);
    command.arg("-batch").arg("-bail");
    for arg in extra_args {
        command.arg(arg);
    }
    command.arg(db_path);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let mut child = command
        .spawn()
        .with_context(|| format!("probe sqlite shell capability with {}", bin.display()))?;
    if !script.is_empty() {
        let mut stdin = child
            .stdin
            .take()
            .context("sqlite shell capability probe stdin unavailable")?;
        use std::io::Write;
        stdin
            .write_all(script.as_bytes())
            .context("write sqlite shell capability probe script")?;
    }
    let status = child
        .wait()
        .context("wait for sqlite shell capability probe")?;
    Ok(status.success())
}

fn shell_help_contains(bin: &Path, needle: &str) -> Result<bool> {
    let output = run_shell_probe(bin, ".help\n.quit\n", &[])?;
    Ok(output.status.success() && output.stdout.contains(needle))
}

fn escape_symbol_option_supported(bin: &Path) -> Result<bool> {
    let output = run_shell_probe(bin, "SELECT char(1);\n", &["-escape", "symbol"])?;
    Ok(output.status.success())
}

fn run_shell_probe(bin: &Path, script: &str, extra_args: &[&str]) -> Result<ShellProbeOutput> {
    let mut command = Command::new(bin);
    command.arg("-batch").arg("-bail");
    for arg in extra_args {
        command.arg(arg);
    }
    command.arg(":memory:");
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("probe sqlite shell with {}", bin.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .context("sqlite shell probe stdin unavailable")?;
    stdin
        .write_all(script.as_bytes())
        .context("write sqlite shell probe script")?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .context("wait for sqlite shell probe")?;
    Ok(ShellProbeOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    })
}

struct ShellProbeOutput {
    status: std::process::ExitStatus,
    stdout: String,
}

fn shell_version_prefix(capabilities: &ShellCapabilities) -> String {
    format!("sqlite3 {}", capabilities.version)
}

#[cfg(unix)]
fn make_removable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("chmod {}", path.display()))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
            let entry = entry.with_context(|| format!("read entry in {}", path.display()))?;
            make_removable(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn make_removable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::read_output_lossy;
    use std::fs;

    #[test]
    fn captured_binary_output_is_decoded_lossily() {
        let path = std::env::temp_dir().join(format!(
            "redline-testing-lossy-output-{}",
            std::process::id()
        ));
        fs::write(&path, [b'a', 0xff, b'b']).expect("write binary output fixture");
        let decoded = read_output_lossy(&path).expect("decode binary output");
        fs::remove_file(path).expect("remove binary output fixture");
        assert_eq!(decoded, "a\u{fffd}b");
    }
}
