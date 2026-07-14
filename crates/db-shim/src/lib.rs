//! `db-shim` — a switchable, rusqlite-shaped database abstraction.
//!
//! Consumers write SQLite-shaped code once (`execute`/`execute_batch`/`query`/`query_row`/
//! `transaction`) and flip the backend by config, so nothing is hard-committed to RedlineDB:
//! - `DB_BACKEND=sqlite`  → local `rusqlite` (bundled SQLite, a WAL file).
//! - `DB_BACKEND=redline` → the central `redlinedb-server` via the proven `redlinedb-client`.
//!
//! Per-project namespacing keeps every project's tables distinct in the ONE central database: SQL
//! uses a `{ns}` token before table names (e.g. `CREATE TABLE {ns}items`), which the shim expands to
//! `<DB_NAMESPACE>_`. All configured via `.env` (`DB_BACKEND` / `DB_DSN` / `DB_NAMESPACE`).

use std::env;

use redlinedb_client::Client;
pub use redlinedb_client::Value;

/// A db-shim error: from either backend, or a config problem.
#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    Redline(redlinedb_client::Error),
    Config(String),
    NotFound,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "sqlite: {e}"),
            Error::Redline(e) => write!(f, "redline: {e}"),
            Error::Config(m) => write!(f, "config: {m}"),
            Error::NotFound => write!(f, "query returned no rows"),
        }
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}
impl From<redlinedb_client::Error> for Error {
    fn from(e: redlinedb_client::Error) -> Self {
        Error::Redline(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

enum Inner {
    Sqlite(rusqlite::Connection),
    Redline(Client),
}

/// A backend-agnostic database handle. Same call surface either way ⇒ switchable.
pub struct Db {
    inner: Inner,
    prefix: String,
}

impl Db {
    /// Open from `.env`: `DB_BACKEND` (default `redline`), `DB_DSN`
    /// (default `redline://127.0.0.1:6033`), `DB_NAMESPACE` (default empty = no prefix).
    pub fn from_env() -> Result<Db> {
        let backend = env::var("DB_BACKEND").unwrap_or_else(|_| "redline".into());
        let dsn = env::var("DB_DSN").unwrap_or_else(|_| "redline://127.0.0.1:6033".into());
        let ns = env::var("DB_NAMESPACE").unwrap_or_default();
        Db::open(&backend, &dsn, &ns)
    }

    /// Open a specific backend. `dsn`: for sqlite a file path (or `:memory:`); for redline a
    /// `redline://host:port` address. `namespace` becomes the `{ns}` table prefix (`""` = none).
    pub fn open(backend: &str, dsn: &str, namespace: &str) -> Result<Db> {
        let prefix = if namespace.is_empty() {
            String::new()
        } else {
            format!("{namespace}_")
        };
        let inner = match backend {
            "sqlite" => {
                let conn = if dsn.is_empty() || dsn == ":memory:" {
                    rusqlite::Connection::open_in_memory()?
                } else {
                    let c = rusqlite::Connection::open(dsn)?;
                    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
                    c
                };
                Inner::Sqlite(conn)
            }
            "redline" => {
                let addr = dsn
                    .strip_prefix("redline://")
                    .or_else(|| dsn.strip_prefix("redlinedb://"))
                    .unwrap_or(dsn);
                Inner::Redline(Client::connect(addr)?)
            }
            other => {
                return Err(Error::Config(format!(
                    "unknown DB_BACKEND {other:?} (expected sqlite|redline)"
                )))
            }
        };
        Ok(Db { inner, prefix })
    }

    /// Expand the `{ns}` table-prefix token.
    fn expand(&self, sql: &str) -> String {
        sql.replace("{ns}", &self.prefix)
    }

    /// Execute a non-parameterized statement; returns rows affected.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        let sql = self.expand(sql);
        match &mut self.inner {
            Inner::Sqlite(c) => Ok(c.execute(&sql, [])? as u64),
            Inner::Redline(c) => Ok(c.execute(&sql)?),
        }
    }

    /// Execute a multi-statement batch (schema/migrations).
    pub fn execute_batch(&mut self, sql: &str) -> Result<()> {
        let sql = self.expand(sql);
        match &mut self.inner {
            Inner::Sqlite(c) => Ok(c.execute_batch(&sql)?),
            Inner::Redline(c) => {
                for stmt in sql.split(';') {
                    if !stmt.trim().is_empty() {
                        c.execute(stmt)?;
                    }
                }
                Ok(())
            }
        }
    }

    /// Execute a parameterized statement.
    pub fn execute_params(&mut self, sql: &str, params: &[Value]) -> Result<()> {
        let sql = self.expand(sql);
        match &mut self.inner {
            Inner::Sqlite(c) => {
                c.execute(&sql, rusqlite::params_from_iter(params.iter().map(to_rusqlite)))?;
                Ok(())
            }
            Inner::Redline(c) => Ok(c.execute_params(&sql, params)?),
        }
    }

    /// Run a query and collect all rows (each row is the column values in order).
    pub fn query(&mut self, sql: &str, params: &[Value]) -> Result<Vec<Vec<Value>>> {
        let sql = self.expand(sql);
        match &mut self.inner {
            Inner::Sqlite(c) => {
                let mut stmt = c.prepare(&sql)?;
                let ncol = stmt.column_count();
                let iter = stmt.query_map(
                    rusqlite::params_from_iter(params.iter().map(to_rusqlite)),
                    |row| {
                        let mut out = Vec::with_capacity(ncol);
                        for i in 0..ncol {
                            out.push(from_rusqlite(row.get_ref(i)?));
                        }
                        Ok(out)
                    },
                )?;
                let mut rows = Vec::new();
                for r in iter {
                    rows.push(r?);
                }
                Ok(rows)
            }
            Inner::Redline(c) => Ok(c.query(&sql, params)?.rows),
        }
    }

    /// Query expecting at least one row; returns the first.
    pub fn query_row(&mut self, sql: &str, params: &[Value]) -> Result<Vec<Value>> {
        let mut rows = self.query(sql, params)?;
        if rows.is_empty() {
            return Err(Error::NotFound);
        }
        Ok(rows.remove(0))
    }

    /// Run `op` inside an immediate transaction; commit on Ok, rollback on Err.
    pub fn transaction<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.begin()?;
        match op(self) {
            Ok(v) => {
                self.commit()?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.rollback();
                Err(e)
            }
        }
    }

    fn begin(&mut self) -> Result<()> {
        match &mut self.inner {
            Inner::Sqlite(c) => Ok(c.execute_batch("BEGIN IMMEDIATE")?),
            Inner::Redline(c) => Ok(c.begin(Some("immediate"))?),
        }
    }
    fn commit(&mut self) -> Result<()> {
        match &mut self.inner {
            Inner::Sqlite(c) => Ok(c.execute_batch("COMMIT")?),
            Inner::Redline(c) => Ok(c.commit()?),
        }
    }
    fn rollback(&mut self) -> Result<()> {
        match &mut self.inner {
            Inner::Sqlite(c) => Ok(c.execute_batch("ROLLBACK")?),
            Inner::Redline(c) => Ok(c.rollback()?),
        }
    }
}

fn to_rusqlite(v: &Value) -> rusqlite::types::Value {
    match v {
        Value::Null => rusqlite::types::Value::Null,
        Value::Integer(i) => rusqlite::types::Value::Integer(*i),
        Value::Real(r) => rusqlite::types::Value::Real(*r),
        Value::Text(s) => rusqlite::types::Value::Text(s.clone()),
        Value::Blob(b) => rusqlite::types::Value::Blob(b.clone()),
    }
}

fn from_rusqlite(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Integer(i),
        ValueRef::Real(r) => Value::Real(r),
        ValueRef::Text(t) => Value::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::Blob(b.to_vec()),
    }
}
