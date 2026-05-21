use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::case::{Case, Profile};

#[derive(Debug, Clone)]
pub struct EngineSpec {
    pub name: String,
    pub bin: PathBuf,
}

#[derive(Debug, Clone)]
pub struct EngineOutput {
    pub engine: String,
    pub status_code: Option<i32>,
    pub elapsed: Duration,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    PercentileFunctions,
    DotCrlf,
    DotDbInfo,
    DotDbTotxt,
    DotRecover,
    EscapeSymbolOption,
}

impl Capability {
    pub fn label(self) -> &'static str {
        match self {
            Self::PercentileFunctions => "SQLITE_PERCENTILE_FUNCTIONS",
            Self::DotCrlf => "CLI_CRLF_COMMAND",
            Self::DotDbInfo => "CLI_DBINFO_COMMAND",
            Self::DotDbTotxt => "CLI_DBTOTXT_COMMAND",
            Self::DotRecover => "CLI_RECOVER_COMMAND",
            Self::EscapeSymbolOption => "CLI_ESCAPE_SYMBOL_OPTION",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::PercentileFunctions => "median()/percentile_cont()",
            Self::DotCrlf => ".crlf",
            Self::DotDbInfo => ".dbinfo",
            Self::DotDbTotxt => ".dbtotxt",
            Self::DotRecover => ".recover",
            Self::EscapeSymbolOption => "-escape symbol",
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
    capabilities: Option<&ShellCapabilities>,
) -> CasePartition {
    let mut partition = CasePartition::default();
    for case in cases {
        let required = required_capabilities(&case);
        let Some(capabilities) = capabilities else {
            partition.runnable.push(case);
            continue;
        };

        if let Some(capability) = required
            .iter()
            .copied()
            .find(|capability| !capabilities.supports(*capability))
        {
            let reason = format!(
                "{} lacks {}",
                shell_version_prefix(capabilities),
                capability.description()
            );
            partition.skipped.push(SkippedCase { case, reason });
        } else {
            partition.runnable.push(case);
        }
    }
    partition
}

pub fn required_capabilities(case: &Case) -> &'static [Capability] {
    match case.id {
        92 => &[Capability::PercentileFunctions],
        134 => &[Capability::DotCrlf],
        154 => &[Capability::DotDbInfo],
        155 => &[Capability::DotDbTotxt],
        156 => &[Capability::DotRecover],
        222 => &[Capability::EscapeSymbolOption],
        _ => &[],
    }
}

pub fn probe_sqlite_shell_capabilities(bin: &Path) -> Result<ShellCapabilities> {
    let version = probe_version(bin)?;
    let memory_db = Path::new(":memory:");
    Ok(ShellCapabilities {
        version,
        percentile_functions: run_sql_script(
            bin,
            memory_db,
            ".mode list\n.headers off\nCREATE TABLE t(x INTEGER); INSERT INTO t VALUES (1), (2), (3);\nSELECT median(x), percentile_cont(0.5) WITHIN GROUP (ORDER BY x) FROM t;\n",
            &[],
        )?,
        dot_crlf: help_contains(bin, ".crlf")?,
        dot_dbinfo: help_contains(bin, ".dbinfo")?,
        dot_dbtotxt: help_contains(bin, ".dbtotxt")?,
        dot_recover: help_contains(bin, ".recover")?,
        escape_symbol_option: cli_help_contains(bin, "-escape symbol")?,
    })
}

impl EngineSpec {
    pub fn new(name: impl Into<String>, bin: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            bin: bin.into(),
        }
    }

    pub fn run_case(&self, case: &Case, tmp_root: &Path) -> Result<EngineOutput> {
        let case_tmp = tmp_root.join(format!(
            "{}-{}-{}",
            case.display_id(),
            sanitize(&self.name),
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
            return self.run_script(case, script, &case_tmp, start);
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
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .with_context(|| format!("spawn {} at {}", self.name, self.bin.display()))?;
        {
            let stdin = child
                .stdin
                .as_mut()
                .context("child stdin unavailable for sqlite parity case")?;
            stdin
                .write_all(replace_tmp(&case.stdin, &case_tmp).as_bytes())
                .with_context(|| format!("write SQL for case {}", case.display_id()))?;
        }
        let output = child
            .wait_with_output()
            .with_context(|| format!("wait for {} case {}", self.name, case.display_id()))?;
        let elapsed = start.elapsed();
        Ok(EngineOutput {
            engine: self.name.clone(),
            status_code: output.status.code(),
            elapsed,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    pub fn sqlite_shell_capabilities(&self) -> Result<Option<ShellCapabilities>> {
        if is_sqlite_shell(&self.name) {
            Ok(Some(probe_sqlite_shell_capabilities(&self.bin)?))
        } else {
            Ok(None)
        }
    }

    fn run_script(
        &self,
        case: &Case,
        script: &str,
        case_tmp: &Path,
        start: Instant,
    ) -> Result<EngineOutput> {
        let script_path = case_tmp.join("case.sh");
        fs::write(&script_path, replace_tmp(script, case_tmp))
            .with_context(|| format!("write script {}", script_path.display()))?;
        let output = Command::new("bash")
            .arg(&script_path)
            .env("SQLITE_BIN", &self.bin)
            .env("SQLITE_PARITY_TMP", case_tmp)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("run script case {}", case.display_id()))?;
        Ok(EngineOutput {
            engine: self.name.clone(),
            status_code: output.status.code(),
            elapsed: start.elapsed(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
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
            let path = case_tmp.join(format!("{}.db", sanitize(engine)));
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

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
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

fn help_contains(bin: &Path, needle: &str) -> Result<bool> {
    let output = Command::new(bin)
        .arg("-help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run {} -help", bin.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains(needle))
}

fn cli_help_contains(bin: &Path, needle: &str) -> Result<bool> {
    let output = Command::new(bin)
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run {} --help", bin.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(stdout.contains(needle) || stderr.contains(needle))
}

fn shell_version_prefix(capabilities: &ShellCapabilities) -> String {
    format!("sqlite3 {}", capabilities.version)
}

pub fn default_tmp_root() -> PathBuf {
    if let Some(path) = std::env::var_os("REDLINEDB_SQLITE_PARITY_TMPDIR")
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }
    if is_writable_dir(Path::new("/dev/shm")) {
        PathBuf::from("/dev/shm/redlinedb-sqlite-parity")
    } else {
        std::env::temp_dir().join("redlinedb-sqlite-parity")
    }
}

fn is_writable_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let probe = path.join(format!(".redlinedb-sqlite-parity-{}", std::process::id()));
    match fs::File::create(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
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

pub fn resolve_engine_bin(
    engine_name: &str,
    sqlite_bin: Option<PathBuf>,
    target_bin: Option<PathBuf>,
) -> Result<PathBuf> {
    if is_sqlite_shell(engine_name) {
        Ok(sqlite_bin.unwrap_or_else(|| PathBuf::from("sqlite3")))
    } else if let Some(target_bin) = target_bin {
        Ok(target_bin)
    } else {
        bail!("--target-bin is required for non-sqlite engine `{engine_name}`");
    }
}
