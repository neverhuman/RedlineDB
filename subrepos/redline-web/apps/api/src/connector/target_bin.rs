//! Target-binary connector.
//!
//! Drives an external SQLite-compatible CLI (e.g. a running `redline` /
//! `redline-core`, or the stock `sqlite3` shell) by spawning it and feeding SQL
//! on stdin — the same `--target-bin` pattern `redline-testing` uses. Output is
//! parsed assuming sqlite-shell semantics (`.mode tabs` / `.headers on`).
//!
//! Only the operations that can be expressed over a generic SQL CLI are
//! supported: [`connection_info`](TargetBinConnector::connection_info),
//! [`schema`](TargetBinConnector::schema) and
//! [`query`](TargetBinConnector::query). Everything else returns a clear
//! [`ConnectorError::Unsupported`] rather than panicking.

use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::{Connector, ConnectorError, PageOptions, is_read_only_sql};
use crate::model::{
    CellValue, ConnectionInfo, ConnectionMode, DbStats, Engine, QueryResult, SchemaObject,
    SchemaObjectKind, SchemaResponse, TablePage, TableSchema, TableSize,
};

/// A connector that shells out to an external SQLite-compatible binary.
#[derive(Debug)]
pub struct TargetBinConnector {
    program: String,
    args: Vec<String>,
    read_only: bool,
    timeout: Duration,
}

impl TargetBinConnector {
    /// Build a connector for `spec`, where `spec` is the binary path optionally
    /// followed by whitespace-separated arguments.
    pub fn new(spec: &str, read_only: bool, timeout: Duration) -> Self {
        let mut parts = spec.split_whitespace().map(str::to_string);
        let (program, args) = if std::path::Path::new(spec).is_file() {
            (spec.to_owned(), Vec::new())
        } else {
            (parts.next().unwrap_or_default(), parts.collect())
        };
        Self {
            program,
            args,
            read_only,
            timeout,
        }
    }

    /// Spawn the target binary, feed `script` on stdin and capture stdout.
    async fn run_script(&self, script: &str) -> Result<String, ConnectorError> {
        if self.program.is_empty() {
            return Err(ConnectorError::Other("no --target-bin configured".into()));
        }
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ConnectorError::Process(format!("spawn {}: {e}", self.program)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(script.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = match tokio::time::timeout(self.timeout, child.wait_with_output()).await {
            Ok(res) => res?,
            Err(_) => return Err(ConnectorError::Timeout),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConnectorError::Process(format!(
                "{} exited with {}: {}",
                self.program,
                output.status,
                stderr.trim()
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Run a single statement in tab-separated, header-on mode.
    async fn run_tabs(&self, sql: &str) -> Result<String, ConnectorError> {
        // Mode selection can reset the shell's header setting.
        let script = format!(".mode tabs\n.headers on\n{}\n", ensure_semicolon(sql));
        self.run_script(&script).await
    }

    /// Probe for a single scalar value (first non-empty output line).
    async fn probe(&self, sql: &str) -> Option<String> {
        let script = format!(".headers off\n.mode list\n{}\n", ensure_semicolon(sql));
        let out = self.run_script(&script).await.ok()?;
        out.lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_string)
    }
}

impl Connector for TargetBinConnector {
    async fn connection_info(&self) -> Result<ConnectionInfo, ConnectorError> {
        let redline = self.probe("SELECT redline_version()").await;
        let sqlite_version = self
            .probe("SELECT sqlite_version()")
            .await
            .unwrap_or_default();
        let (engine, engine_version) = match redline {
            Some(v) if !v.is_empty() => (Engine::Redline, Some(v)),
            _ => (Engine::Sqlite, None),
        };
        Ok(ConnectionInfo {
            mode: ConnectionMode::TargetBin,
            engine,
            path: self.program.clone(),
            read_only: self.read_only,
            sqlite_version,
            engine_version,
            size_bytes: 0,
        })
    }

    async fn schema(&self) -> Result<SchemaResponse, ConnectorError> {
        let out = self
            .run_tabs(
                "SELECT name, type, sql FROM sqlite_master \
                 WHERE type IN ('table','view','index','trigger') \
                 AND name NOT LIKE 'sqlite_%' ORDER BY type, name",
            )
            .await?;
        let (_, rows) = parse_tabs(&out);
        let objects = rows
            .into_iter()
            .filter_map(|row| {
                let name = row.first().and_then(cell_text)?;
                let kind = row.get(1).and_then(cell_text).unwrap_or_default();
                let sql = row.get(2).and_then(cell_text);
                Some(SchemaObject {
                    name,
                    kind: parse_kind(&kind),
                    sql,
                    row_count: None,
                })
            })
            .collect();
        Ok(SchemaResponse { objects })
    }

    async fn table_schema(&self, _name: &str) -> Result<TableSchema, ConnectorError> {
        Err(ConnectorError::Unsupported(
            "table schema is not available over --target-bin".into(),
        ))
    }

    async fn table_page(
        &self,
        _name: &str,
        _opts: &PageOptions,
    ) -> Result<TablePage, ConnectorError> {
        Err(ConnectorError::Unsupported(
            "table paging is not available over --target-bin".into(),
        ))
    }

    async fn query(&self, sql: &str, max_rows: i64) -> Result<QueryResult, ConnectorError> {
        if self.read_only && !is_read_only_sql(sql) {
            return Err(ConnectorError::ReadOnly(
                "connection is read-only; only SELECT/EXPLAIN/PRAGMA/WITH/VALUES allowed".into(),
            ));
        }
        let start = Instant::now();
        let out = self.run_tabs(sql).await?;
        let elapsed = start.elapsed();

        let (columns, mut rows) = parse_tabs(&out);
        let cap = max_rows.max(0) as usize;
        let truncated = rows.len() > cap;
        if truncated {
            rows.truncate(cap);
        }
        let row_count = rows.len() as i64;
        Ok(QueryResult {
            columns,
            rows,
            row_count,
            rows_affected: None,
            elapsed_ms: elapsed.as_secs_f64() * 1000.0,
            truncated,
        })
    }

    async fn db_stats(&self) -> Result<DbStats, ConnectorError> {
        Err(ConnectorError::Unsupported(
            "db stats are not available over --target-bin".into(),
        ))
    }

    async fn table_sizes(&self) -> Result<Vec<TableSize>, ConnectorError> {
        Err(ConnectorError::Unsupported(
            "table sizes are not available over --target-bin".into(),
        ))
    }
}

/// Append a trailing semicolon if the statement lacks one.
fn ensure_semicolon(sql: &str) -> String {
    let trimmed = sql.trim_end();
    if trimmed.ends_with(';') {
        trimmed.to_string()
    } else {
        format!("{trimmed};")
    }
}

/// Parse sqlite-shell `.mode tabs` `.headers on` output into columns and rows.
fn parse_tabs(out: &str) -> (Vec<String>, Vec<Vec<CellValue>>) {
    let mut lines = out.lines().filter(|l| !l.is_empty());
    let header = match lines.next() {
        Some(h) => h,
        None => return (Vec::new(), Vec::new()),
    };
    let columns: Vec<String> = header.split('\t').map(str::to_string).collect();
    let rows = lines
        .map(|line| line.split('\t').map(parse_cell).collect())
        .collect();
    (columns, rows)
}

/// Best-effort scalar inference for a CLI cell (int, float, else string/null).
fn parse_cell(s: &str) -> CellValue {
    if s.is_empty() {
        return CellValue::Null;
    }
    if let Ok(i) = s.parse::<i64>() {
        return CellValue::from(i);
    }
    if let Ok(f) = s.parse::<f64>()
        && let Some(n) = serde_json::Number::from_f64(f)
    {
        return CellValue::Number(n);
    }
    CellValue::String(s.to_string())
}

/// Extract a plain string from a parsed cell, if any.
fn cell_text(value: &CellValue) -> Option<String> {
    match value {
        CellValue::Null => None,
        CellValue::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

fn parse_kind(kind: &str) -> SchemaObjectKind {
    match kind {
        "view" => SchemaObjectKind::View,
        "index" => SchemaObjectKind::Index,
        "trigger" => SchemaObjectKind::Trigger,
        _ => SchemaObjectKind::Table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_path_can_contain_spaces() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("database shell");
        std::fs::write(&path, "").expect("executable fixture");
        let connector =
            TargetBinConnector::new(path.to_str().unwrap(), false, Duration::from_secs(1));
        assert_eq!(connector.program, path.to_str().unwrap());
        assert!(connector.args.is_empty());
    }

    #[test]
    fn parses_tab_output() {
        let out = "id\tname\n1\tAda\n2\tLinus\n";
        let (cols, rows) = parse_tabs(out);
        assert_eq!(cols, vec!["id", "name"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], serde_json::json!(1));
        assert_eq!(rows[1][1], serde_json::json!("Linus"));
    }

    #[test]
    fn empty_cell_is_null() {
        assert_eq!(parse_cell(""), CellValue::Null);
        assert_eq!(parse_cell("3.5"), serde_json::json!(3.5));
        assert_eq!(parse_cell("hi"), serde_json::json!("hi"));
    }

    #[tokio::test]
    async fn unsupported_ops_error_cleanly() {
        let c = TargetBinConnector::new("/bin/true", false, Duration::from_secs(1));
        assert!(matches!(
            c.db_stats().await,
            Err(ConnectorError::Unsupported(_))
        ));
        assert!(matches!(
            c.table_schema("x").await,
            Err(ConnectorError::Unsupported(_))
        ));
    }
}
