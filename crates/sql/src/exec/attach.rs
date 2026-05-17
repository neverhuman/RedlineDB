//! ATTACH / DETACH DATABASE — minimal alias-keyed multi-DB resolution.
//!
//! In SQLite, `ATTACH DATABASE 'file' AS alias` opens a second database
//! image that can then be referenced via `alias.table`. Our minimum
//! drop-in implementation keeps an in-process alias map keyed by string
//! name and re-uses the existing single-DB engine for each attached
//! database. The "main" alias is always present and refers to the engine
//! that the connection was opened against.
//!
//! Storage is **per-connection** for simplicity (single-thread access).
//! Cross-database SELECT works at the parser/planner layer by resolving
//! `alias.table` against the attached map before falling back to the
//! current connection's catalog.
//!
//! This module owns the alias map; the parser routes ATTACH/DETACH plans
//! through it via [`AttachPlan::apply`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::connection::Connection;
use crate::error::{Error, Result};

/// A single ATTACH / DETACH directive emitted by the parser.
#[derive(Debug, Clone)]
pub enum AttachPlan {
    Attach { path: PathBuf, alias: Arc<str> },
    Detach { alias: Arc<str> },
}

/// Alias map shared inside a single [`Connection`]. The empty alias
/// (`""`) is reserved for the "main" database and is never inserted by
/// `attach`; it is implicit.
#[derive(Debug, Default)]
pub struct AttachMap {
    inner: RwLock<HashMap<String, PathBuf>>,
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
        let mut guard = self
            .inner
            .write()
            .map_err(|_| Error::TransactionState("attach map poisoned"))?;
        if guard.contains_key(&lower) {
            return Err(Error::UnsupportedSql(format!(
                "database '{alias}' is already in use"
            )));
        }
        guard.insert(lower, path);
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
            return Err(Error::UnknownTable(format!(
                "no such database: {alias}"
            )));
        }
        Ok(())
    }

    /// Inspect whether `alias` is currently bound; `main` is always present.
    /// Reserved for the cross-database planner path that consumes the alias
    /// map; kept on the public surface so the parser side can validate
    /// without a round-trip through `path` first.
    #[allow(dead_code)]
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

    /// Return the on-disk path bound to `alias`, if any. Reserved for the
    /// cross-database planner path that resolves `alias.table` references.
    #[allow(dead_code)]
    pub fn path(&self, alias: &str) -> Option<PathBuf> {
        let lower = alias.to_ascii_lowercase();
        self.inner.read().ok().and_then(|g| g.get(&lower).cloned())
    }

    /// List every alias currently visible from this connection, including
    /// the implicit `main` alias. Reserved for `PRAGMA database_list`.
    #[allow(dead_code)]
    pub fn aliases(&self) -> Vec<String> {
        let mut out = vec!["main".to_owned()];
        if let Ok(g) = self.inner.read() {
            out.extend(g.keys().cloned());
        }
        out
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
