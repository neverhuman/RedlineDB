use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use rusqlite::Connection;
use tempfile::NamedTempFile;

use super::case::Case;

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
    let probe_db = seed_probe_db()?;
    let db_path = probe_db.path();
    Ok(ShellCapabilities {
        version,
        percentile_functions: run_sql_script(
            bin,
            db_path,
            ".mode list\n.headers off\nSELECT median(x), percentile_cont(0.5) WITHIN GROUP (ORDER BY x) FROM t;\n",
            &[],
        )?,
        dot_crlf: run_sql_script(bin, db_path, ".crlf on\n", &[])?,
        dot_dbinfo: run_sql_script(bin, db_path, ".dbinfo\n", &[])?,
        dot_dbtotxt: run_sql_script(bin, db_path, ".dbtotxt\n", &[])?,
        dot_recover: run_sql_script(bin, db_path, ".recover\n", &[])?,
        escape_symbol_option: run_sql_script(bin, db_path, ".quit\n", &["-escape", "symbol"])?,
    })
}

fn seed_probe_db() -> Result<NamedTempFile> {
    let temp = NamedTempFile::new().context("create sqlite parity capability probe database")?;
    let conn = Connection::open(temp.path()).with_context(|| {
        format!(
            "open sqlite parity capability probe database {}",
            temp.path().display()
        )
    })?;
    conn.execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (1), (2), (3);")
        .context("seed sqlite parity capability probe database")?;
    Ok(temp)
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

fn shell_version_prefix(capabilities: &ShellCapabilities) -> String {
    format!("sqlite3 {}", capabilities.version)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::sqlite_parity::case::{Case, Priority, Profile};

    fn case(id: usize) -> Case {
        Case {
            id,
            folder: String::from("CASE"),
            name: String::from("CASE"),
            category: String::from("CAT"),
            priority: Priority::P3,
            profile: Profile::Memory,
            kind: String::from("sql"),
            description: String::new(),
            status: String::from("active"),
            db: String::from(":memory:"),
            args: vec![],
            stdin: String::new(),
            expected_exit: 0,
            compare_stdout: true,
            expected_stdout: None,
            expected_stdout_contains: vec![],
            expected_stderr_contains: vec![],
            expected_combined_contains: vec![],
            files: vec![],
            script: None,
            notes: String::new(),
        }
    }

    #[test]
    fn documented_cases_require_the_expected_capabilities() {
        assert_eq!(
            required_capabilities(&case(92)),
            &[Capability::PercentileFunctions]
        );
        assert_eq!(required_capabilities(&case(134)), &[Capability::DotCrlf]);
        assert_eq!(required_capabilities(&case(154)), &[Capability::DotDbInfo]);
        assert_eq!(required_capabilities(&case(155)), &[Capability::DotDbTotxt]);
        assert_eq!(required_capabilities(&case(156)), &[Capability::DotRecover]);
        assert_eq!(
            required_capabilities(&case(222)),
            &[Capability::EscapeSymbolOption]
        );
        assert!(required_capabilities(&case(1)).is_empty());
    }

    #[test]
    fn partitioning_skips_only_unsupported_capabilities() {
        let cases = vec![
            case(92),
            case(134),
            case(154),
            case(155),
            case(156),
            case(222),
        ];
        let capabilities = ShellCapabilities::default();
        let partition = partition_cases(cases, Some(&capabilities));
        assert!(partition.runnable.is_empty());
        assert_eq!(partition.skipped.len(), 6);
    }

    #[test]
    fn current_system_sqlite3_lacks_documented_optional_shell_capabilities() {
        let capabilities = probe_sqlite_shell_capabilities(Path::new("sqlite3"))
            .expect("probe current sqlite3 shell");
        assert!(!capabilities.percentile_functions);
        assert!(!capabilities.dot_crlf);
        assert!(!capabilities.dot_dbinfo);
        assert!(!capabilities.dot_dbtotxt);
        assert!(!capabilities.dot_recover);
        assert!(!capabilities.escape_symbol_option);
    }
}
