//! SQLite-file connector backed by bundled `rusqlite`.
//!
//! Opens any SQLite database file (or `:memory:`) and serves schema, paging,
//! ad-hoc queries and storage statistics. Identifier interpolation is always
//! validated against the live schema and quoted, so table/column names taken
//! from the request can never inject SQL.
//!
//! A brand-new or empty database is seeded with a tiny demo schema so the UI is
//! useful out of the box (skipped when opened read-only).

use std::time::{Duration, Instant};

use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags, OptionalExtension};

use super::sqlite_support::{
    column_names, count_rows, dbstat_available, is_empty, is_interrupt, parse_kind, run_query,
    seed_demo, value_to_json,
};
use super::{Connector, ConnectorError, PageOptions, is_read_only_sql, quote_ident};
use crate::model::{
    CellValue, ColumnInfo, ConnectionInfo, ConnectionMode, DbStats, Engine, QueryResult,
    SchemaObject, SchemaObjectKind, SchemaResponse, TablePage, TableSchema, TableSize,
};

/// A connector that opens a SQLite database file directly.
#[derive(Debug)]
pub struct SqliteConnector {
    path: String,
    read_only: bool,
    query_timeout: Duration,
    conn: Mutex<Connection>,
}

impl SqliteConnector {
    /// Open `path` (`:memory:` for in-memory). When not read-only and the
    /// database is empty, a small demo schema is seeded.
    pub fn open(
        path: &str,
        read_only: bool,
        query_timeout: Duration,
    ) -> Result<Self, ConnectorError> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else if read_only {
            Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?
        } else {
            Connection::open(path)?
        };

        let me = Self {
            path: path.to_string(),
            read_only,
            query_timeout,
            conn: Mutex::new(conn),
        };

        if !read_only {
            let conn = me.conn.lock();
            if is_empty(&conn)? {
                seed_demo(&conn)?;
            }
        }

        Ok(me)
    }

    fn size_on_disk(&self, conn: &Connection) -> u64 {
        if self.path != ":memory:"
            && let Ok(meta) = std::fs::metadata(&self.path)
        {
            return meta.len();
        }
        let page_count: i64 = conn
            .query_row("PRAGMA page_count", [], |r| r.get(0))
            .unwrap_or(0);
        let page_size: i64 = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap_or(0);
        page_count.max(0) as u64 * page_size.max(0) as u64
    }

    fn wal_bytes(&self) -> u64 {
        if self.path == ":memory:" {
            return 0;
        }
        std::fs::metadata(format!("{}-wal", self.path))
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

impl Connector for SqliteConnector {
    async fn connection_info(&self) -> Result<ConnectionInfo, ConnectorError> {
        let conn = self.conn.lock();
        let sqlite_version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
        let (engine, engine_version) =
            match conn.query_row("SELECT redline_version()", [], |r| r.get::<_, String>(0)) {
                Ok(v) => (Engine::Redline, Some(v)),
                Err(_) => (Engine::Sqlite, None),
            };
        let size_bytes = self.size_on_disk(&conn);
        Ok(ConnectionInfo {
            mode: ConnectionMode::SqliteFile,
            engine,
            path: self.path.clone(),
            read_only: self.read_only,
            sqlite_version,
            engine_version,
            size_bytes,
        })
    }

    async fn schema(&self) -> Result<SchemaResponse, ConnectorError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT name, type, sql FROM sqlite_master \
             WHERE type IN ('table','view','index','trigger') \
             AND name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )?;
        let raw: Vec<(String, String, Option<String>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let mut objects = Vec::with_capacity(raw.len());
        for (name, kind_str, sql) in raw {
            let kind = parse_kind(&kind_str);
            let row_count = match kind {
                SchemaObjectKind::Table | SchemaObjectKind::View => count_rows(&conn, &name),
                _ => None,
            };
            objects.push(SchemaObject {
                name,
                kind,
                sql,
                row_count,
            });
        }
        Ok(SchemaResponse { objects })
    }

    async fn table_schema(&self, name: &str) -> Result<TableSchema, ConnectorError> {
        let conn = self.conn.lock();
        let kind: Option<String> = conn
            .query_row(
                "SELECT type FROM sqlite_master WHERE name=?1 AND type IN ('table','view')",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        let kind = kind.ok_or_else(|| ConnectorError::NotFound(format!("table {name}")))?;

        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(name)))?;
        let columns: Vec<ColumnInfo> = stmt
            .query_map([], |r| {
                Ok(ColumnInfo {
                    name: r.get::<_, String>(1)?,
                    type_: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    not_null: r.get::<_, i64>(3)? != 0,
                    default_value: r.get::<_, Option<String>>(4)?,
                    pk: r.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let mut idx_stmt = conn.prepare(&format!("PRAGMA index_list({})", quote_ident(name)))?;
        let indexes: Vec<String> = idx_stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;
        drop(idx_stmt);

        let row_count = count_rows(&conn, name);
        Ok(TableSchema {
            name: name.to_string(),
            kind,
            columns,
            row_count,
            indexes,
        })
    }

    async fn table_page(
        &self,
        name: &str,
        opts: &PageOptions,
    ) -> Result<TablePage, ConnectorError> {
        let conn = self.conn.lock();
        let exists: Option<String> = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE name=?1 AND type IN ('table','view')",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(ConnectorError::NotFound(format!("table {name}")));
        }

        let cols = column_names(&conn, name)?;
        let order_clause = match &opts.order_by {
            Some(ob) => {
                if !cols.iter().any(|c| c == ob) {
                    return Err(ConnectorError::InvalidArgument(format!(
                        "unknown orderBy column: {ob}"
                    )));
                }
                let dir = match opts.dir.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    Some("desc") => "DESC",
                    Some("asc") | None => "ASC",
                    Some(other) => {
                        return Err(ConnectorError::InvalidArgument(format!(
                            "invalid dir: {other}"
                        )));
                    }
                };
                format!(" ORDER BY {} {}", quote_ident(ob), dir)
            }
            None => String::new(),
        };

        let limit = opts.limit.max(0);
        let offset = opts.offset.max(0);
        let total = count_rows(&conn, name);

        let sql = format!(
            "SELECT * FROM {}{} LIMIT ?1 OFFSET ?2",
            quote_ident(name),
            order_clause
        );
        let mut stmt = conn.prepare(&sql)?;
        let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let col_count = columns.len();
        let mut rows_out: Vec<Vec<CellValue>> = Vec::new();
        let mut rows = stmt.query([limit, offset])?;
        while let Some(row) = rows.next()? {
            let mut record = Vec::with_capacity(col_count);
            for i in 0..col_count {
                record.push(value_to_json(row.get_ref(i)?));
            }
            rows_out.push(record);
        }

        Ok(TablePage {
            name: name.to_string(),
            columns,
            rows: rows_out,
            total,
            limit,
            offset,
        })
    }

    async fn query(&self, sql: &str, max_rows: i64) -> Result<QueryResult, ConnectorError> {
        if self.read_only && !is_read_only_sql(sql) {
            return Err(ConnectorError::ReadOnly(
                "connection is read-only; only SELECT/EXPLAIN/PRAGMA/WITH/VALUES allowed".into(),
            ));
        }

        let conn = self.conn.lock();
        let _ = conn.busy_timeout(self.query_timeout);

        // Enforce the per-query timeout out-of-band: a watcher thread interrupts
        // the connection if the query outruns the deadline. Signalling it via a
        // channel on completion avoids interrupting a query that finished first.
        let handle = conn.get_interrupt_handle();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let timeout = self.query_timeout;
        let watcher = std::thread::spawn(move || {
            if done_rx.recv_timeout(timeout).is_err() {
                handle.interrupt();
            }
        });

        let start = Instant::now();
        let res = run_query(&conn, sql, max_rows);
        let elapsed = start.elapsed();

        let _ = done_tx.send(());
        let _ = watcher.join();

        let mut result = res.map_err(|e| {
            if is_interrupt(&e) {
                ConnectorError::Timeout
            } else {
                ConnectorError::Sqlite(e)
            }
        })?;
        result.elapsed_ms = elapsed.as_secs_f64() * 1000.0;
        Ok(result)
    }

    async fn db_stats(&self) -> Result<DbStats, ConnectorError> {
        let conn = self.conn.lock();
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        let size_bytes = self.size_on_disk(&conn);
        let wal_bytes = self.wal_bytes();
        Ok(DbStats {
            size_bytes,
            page_count,
            page_size,
            freelist_count,
            wal_bytes,
        })
    }

    async fn table_sizes(&self) -> Result<Vec<TableSize>, ConnectorError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let names: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);

        let dbstat = dbstat_available(&conn);
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let row_count = count_rows(&conn, &name);
            let bytes = if dbstat {
                conn.query_row(
                    "SELECT sum(pgsize) FROM dbstat WHERE name=?1",
                    [&name],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
            } else {
                None
            };
            out.push(TableSize {
                name,
                row_count,
                bytes,
            });
        }
        Ok(out)
    }
}
