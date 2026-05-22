//! ATTACH / DETACH DATABASE — per-connection alias map with sidecar engines.
//!
//! In SQLite, `ATTACH DATABASE 'file' AS alias` opens a second database
//! image that can then be referenced via `alias.table`. We open a real
//! sidecar [`Database`] per alias and reuse the single-DB engine for
//! reads through it.
//!
//! Storage is **per-connection** for simplicity (single-thread access).
//! Cross-database SELECT resolves `alias.table` against the attached
//! map in [`crate::exec::cross_db::try_resolve_cross_db_bound_table`]
//! by materializing rows from the sidecar engine at bind time, mirroring
//! the view pattern. Cross-database writes are rejected with a clear
//! error pending follow-up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::DbOptions;
use crate::connection::{Connection, Database};
use crate::error::{Error, Result};

/// A single ATTACH / DETACH directive emitted by the parser.
#[derive(Debug, Clone)]
pub enum AttachPlan {
    Attach { path: PathBuf, alias: Arc<str> },
    Detach { alias: Arc<str> },
}

/// Per-alias entry: the on-disk path and the opened sidecar engine.
#[derive(Clone)]
struct AttachedDb {
    #[allow(dead_code)] // surfaced via AttachMap::path for PRAGMA database_list
    path: PathBuf,
    db: Arc<Database>,
}

/// Alias map shared inside a single [`Connection`]. The empty alias
/// (`""`) is reserved for the "main" database and is never inserted by
/// `attach`; it is implicit.
#[derive(Default)]
pub struct AttachMap {
    inner: RwLock<HashMap<String, AttachedDb>>,
}

impl std::fmt::Debug for AttachMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let aliases: Vec<String> = self
            .inner
            .read()
            .map(|g| g.keys().cloned().collect())
            .unwrap_or_default();
        f.debug_struct("AttachMap")
            .field("aliases", &aliases)
            .finish()
    }
}

impl AttachMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&self, alias: &str, path: PathBuf) -> Result<()> {
        let lower = alias.to_ascii_lowercase();
        if lower == "main" || lower == "temp" {
            return Err(Error::UnsupportedSql(format!(
                "alias '{alias}' is reserved by the engine"
            )));
        }
        let db = open_or_create(&path)?;
        let mut guard = self
            .inner
            .write()
            .map_err(|_| Error::TransactionState("attach map poisoned"))?;
        if guard.contains_key(&lower) {
            return Err(Error::UnsupportedSql(format!(
                "database '{alias}' is already in use"
            )));
        }
        guard.insert(lower, AttachedDb { path, db });
        Ok(())
    }

    pub fn detach(&self, alias: &str) -> Result<()> {
        let lower = alias.to_ascii_lowercase();
        if lower == "main" || lower == "temp" {
            return Err(Error::UnsupportedSql(format!(
                "cannot detach reserved database '{alias}'"
            )));
        }
        let mut guard = self
            .inner
            .write()
            .map_err(|_| Error::TransactionState("attach map poisoned"))?;
        if guard.remove(&lower).is_none() {
            return Err(Error::UnknownTable(format!("no such database: {alias}")));
        }
        Ok(())
    }

    /// True if `alias` is currently bound; `main` is always present.
    #[allow(dead_code)] // public-surface helper for future PRAGMA database_list
    pub fn contains(&self, alias: &str) -> bool {
        let lower = alias.to_ascii_lowercase();
        if lower == "main" {
            return true;
        }
        self.inner
            .read()
            .map(|g| g.contains_key(&lower))
            .unwrap_or(false)
    }

    /// On-disk path bound to `alias`, if any.
    #[allow(dead_code)] // public-surface helper for future PRAGMA database_list
    pub fn path(&self, alias: &str) -> Option<PathBuf> {
        let lower = alias.to_ascii_lowercase();
        self.inner
            .read()
            .ok()
            .and_then(|g| g.get(&lower).map(|e| e.path.clone()))
    }

    /// Sidecar [`Database`] handle bound to `alias`, if any. `main` is
    /// not stored here — callers must check the alias is non-main first.
    pub fn database(&self, alias: &str) -> Option<Arc<Database>> {
        let lower = alias.to_ascii_lowercase();
        self.inner
            .read()
            .ok()
            .and_then(|g| g.get(&lower).map(|e| Arc::clone(&e.db)))
    }

    /// Every alias currently visible from this connection, including
    /// the implicit `main` alias. Used by `PRAGMA database_list`.
    #[allow(dead_code)] // wired up by subtask D PRAGMA additions
    pub fn aliases(&self) -> Vec<String> {
        let mut out = vec!["main".to_owned()];
        if let Ok(g) = self.inner.read() {
            out.extend(g.keys().cloned());
        }
        out
    }
}

fn open_or_create(path: &Path) -> Result<Arc<Database>> {
    if path == Path::new(":memory:") {
        return Database::create_in_memory(DbOptions::default());
    }
    if path.exists() {
        Database::open(path, DbOptions::default())
    } else {
        Database::create(path, DbOptions::default())
    }
}

/// Apply an ATTACH/DETACH directive to the per-connection alias map.
pub(crate) fn apply_attach_plan(conn: &Connection, plan: &AttachPlan) -> Result<()> {
    let map = conn.attach_map();
    match plan {
        AttachPlan::Attach { path, alias } => map.attach(alias, path.clone()),
        AttachPlan::Detach { alias } => map.detach(alias),
    }
}
