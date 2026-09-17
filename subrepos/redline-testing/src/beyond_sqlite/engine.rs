//! Postgres reference resolver + psql subprocess driver.
//!
//! Resolution order:
//!   1. `REDLINE_TESTING_POSTGRES_URL` env var (libpq-style URL or DSN).
//!   2. With `--features pg-embedded` + `REDLINE_TESTING_POSTGRES_EMBEDDED=on`:
//!      spawn an embedded Postgres on /dev/shm via the `postgresql_embedded`
//!      crate. (Currently a stub awaiting the optional dep being wired in.)
//!   3. Fallback to `Unavailable` with a diagnostic explaining what's
//!      missing.
//!
//! Hard constraint: SQLite-parity must NEVER depend on Postgres being
//! available. This resolver is only called from the beyond-SQLite oracle.

use std::env;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

/// Postgres reference target. Carries enough information to invoke `psql`
/// against the reference, plus the version string for provenance.
#[derive(Debug, Clone)]
pub struct PostgresReference {
    pub bin: PathBuf,
    pub connection: PgConnection,
    #[allow(dead_code)]
    pub version: String,
}

#[derive(Debug, Clone)]
pub enum PgConnection {
    /// Connect by URL (libpq DSN or env-supplied).
    Url(String),
    /// Connect by Unix socket + port + user + dbname.
    Socket {
        socket_dir: PathBuf,
        port: u16,
        user: String,
        dbname: String,
    },
}

impl PgConnection {
    fn as_psql_args(&self) -> Vec<String> {
        match self {
            Self::Url(url) => vec![url.clone()],
            Self::Socket {
                socket_dir,
                port,
                user,
                dbname,
            } => vec![
                "-h".to_owned(),
                socket_dir.to_string_lossy().into_owned(),
                "-p".to_owned(),
                port.to_string(),
                "-U".to_owned(),
                user.clone(),
                "-d".to_owned(),
                dbname.clone(),
            ],
        }
    }

    /// Render as a libpq URL for env-var passing to subprocesses.
    #[allow(dead_code)]
    pub fn as_url_string(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Socket {
                socket_dir,
                port,
                user,
                dbname,
            } => format!(
                "postgresql://{user}@/{dbname}?host={}&port={port}",
                socket_dir.display()
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResolveOutcome {
    Configured(PostgresReference),
    Unavailable { reason: String },
}

/// Resolve the Postgres reference engine. NEVER errors; "couldn't find a
/// usable Postgres" is reported as `Unavailable` with a diagnostic so the
/// caller can downgrade the affected cases to `skipped` instead of failing
/// the whole suite.
pub fn resolve() -> ResolveOutcome {
    let psql_bin = match psql_binary() {
        Some(bin) => bin,
        None => {
            return ResolveOutcome::Unavailable {
                reason: "psql not found on PATH".to_owned(),
            };
        }
    };

    if let Ok(url) = env::var("REDLINE_TESTING_POSTGRES_URL")
        && !url.trim().is_empty()
    {
        match probe(&psql_bin, &PgConnection::Url(url.clone())) {
            Ok(version) => {
                return ResolveOutcome::Configured(PostgresReference {
                    bin: psql_bin,
                    connection: PgConnection::Url(url),
                    version,
                });
            }
            Err(err) => {
                return ResolveOutcome::Unavailable {
                    reason: format!("REDLINE_TESTING_POSTGRES_URL probe failed: {err}"),
                };
            }
        }
    }

    // Default fallback: the dev-time PG cluster on /dev/shm if the operator
    // started it via the documented one-liner (see CONTRIBUTING). We probe
    // a small set of conventional locations before giving up.
    let conventional = [("/dev/shm/redline-pg-sock", 5433, "ubuntu", "postgres")];
    for (sock, port, user, db) in conventional {
        let conn = PgConnection::Socket {
            socket_dir: PathBuf::from(sock),
            port,
            user: user.to_owned(),
            dbname: db.to_owned(),
        };
        if let Ok(version) = probe(&psql_bin, &conn) {
            return ResolveOutcome::Configured(PostgresReference {
                bin: psql_bin,
                connection: conn,
                version,
            });
        }
    }

    #[cfg(feature = "pg-embedded")]
    if env::var("REDLINE_TESTING_POSTGRES_EMBEDDED").as_deref() == Ok("on") {
        // Embedded path: postgresql_embedded crate. Currently a stub; the
        // dep is intentionally NOT pulled in by commit 5 to keep the default
        // build deps unchanged.
        return ResolveOutcome::Unavailable {
            reason: "pg-embedded feature enabled but dep not yet wired in".to_owned(),
        };
    }

    ResolveOutcome::Unavailable {
        reason: "no REDLINE_TESTING_POSTGRES_URL and no Postgres on /dev/shm:5433".to_owned(),
    }
}

fn psql_binary() -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|dir| dir.join("psql"))
        .find(|p| p.is_file())
}

fn probe(psql: &std::path::Path, conn: &PgConnection) -> Result<String> {
    let output = invoke_psql(psql, conn, "SELECT version();", &[], Duration::from_secs(3))
        .context("invoke psql --version probe")?;
    if output.exit_code != 0 {
        bail!(
            "psql probe failed exit={} stderr={}",
            output.exit_code,
            output.stderr
        );
    }
    Ok(output.stdout.lines().next().unwrap_or("").trim().to_owned())
}

/// Result of running a single psql invocation.
#[derive(Debug, Clone)]
pub struct PsqlOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run psql against the connection with the given SQL stdin and optional
/// `-c` pre-commands (used for `SET timezone = 'UTC'` etc.). The unaligned
/// + tuples-only + null-NULL formatting mirrors SQLite's `.mode list` /
/// `.nullvalue NULL` so output is byte-comparable through the normalizer
/// pipeline.
pub fn invoke_psql(
    psql: &std::path::Path,
    conn: &PgConnection,
    stdin: &str,
    extra_set_commands: &[(String, String)],
    timeout: Duration,
) -> Result<PsqlOutput> {
    let mut command = Command::new(psql);
    command
        .arg("-q")
        .arg("-A")
        .arg("-t")
        .arg("-X")
        .arg("-v")
        .arg("ON_ERROR_STOP=1")
        .arg("-F")
        .arg("|");
    for arg in conn.as_psql_args() {
        command.arg(arg);
    }
    // Use -P (pset option) rather than -c "\pset ..." so the formatting
    // applies to the SQL we then feed on stdin. -c switches psql into
    // single-command mode and stops reading stdin afterwards.
    command
        .arg("-P")
        .arg("null=NULL")
        .arg("-P")
        .arg("format=unaligned")
        .arg("-P")
        .arg("tuples_only=on")
        .arg("-P")
        .arg("fieldsep=|")
        .arg("-P")
        .arg("border=0");
    // NOTE: pg_settings used to be passed via `-c "SET key = value"`, but
    // psql switches into single-command mode the moment it sees any `-c`
    // argument and stops reading stdin afterwards — so the actual SELECT
    // never ran and the case appeared as a phantom stdout-diff failure.
    // We now prepend `SET key = value;` statements to the stdin instead;
    // psql runs them in the same session as the case's SQL.
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("spawn psql at {}", psql.display()))?;
    {
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("no stdin pipe for psql child"))?;
        for (key, value) in extra_set_commands {
            let line = format!("SET {key} = {value};\n");
            child_stdin
                .write_all(line.as_bytes())
                .context("write psql SET prelude")?;
        }
        child_stdin
            .write_all(stdin.as_bytes())
            .context("write psql stdin")?;
    }
    // Wall-clock timeout: poll try_wait every 50ms until done or expired.
    let started = std::time::Instant::now();
    let mut completed = false;
    loop {
        match child.try_wait()? {
            Some(_) => {
                completed = true;
                break;
            }
            None => {
                if started.elapsed() >= timeout {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    if !completed {
        let _ = child.kill();
        let _ = child.wait();
        bail!("psql timed out after {:?}", timeout);
    }
    let out = child.wait_with_output().context("psql wait_with_output")?;
    Ok(PsqlOutput {
        stdout: normalize_newlines(&String::from_utf8_lossy(&out.stdout)),
        stderr: normalize_newlines(&String::from_utf8_lossy(&out.stderr)),
        exit_code: out.status.code().unwrap_or(-1),
    })
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}
