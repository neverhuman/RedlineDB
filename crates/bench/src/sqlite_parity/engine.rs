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

fn is_sqlite_shell(engine_name: &str) -> bool {
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
