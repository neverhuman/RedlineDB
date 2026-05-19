#![allow(missing_docs)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::async_row::AsyncRow;
use crate::{
    BeginMode, Database, Error, ErrorCode, ExecuteSummary, OpenOptions, PoolBuilder, Result, Value,
};

#[derive(Clone)]
pub struct Pool {
    pub(crate) inner: Arc<PoolInner>,
}

pub(crate) struct PoolInner {
    pub(crate) db: Database,
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) busy_timeout: Duration,
    pub(crate) max_connections: usize,
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("max_permits", &self.inner.semaphore.available_permits())
            .field("busy_timeout", &self.inner.busy_timeout)
            .finish()
    }
}

impl Pool {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let db = tokio::task::spawn_blocking(move || Database::create(path))
            .await
            .map_err(join_err)??;
        Self::from_database(db)
    }

    pub async fn open_in_memory() -> Result<Self> {
        let db = tokio::task::spawn_blocking(|| Database::create_in_memory(OpenOptions::default()))
            .await
            .map_err(join_err)??;
        Self::from_database(db)
    }

    pub fn from_database(db: Database) -> Result<Self> {
        PoolBuilder::default().database(db).build()
    }

    pub fn builder() -> PoolBuilder {
        PoolBuilder::default()
    }

    pub fn database(&self) -> &Database {
        &self.inner.db
    }

    pub fn max_connections(&self) -> usize {
        self.inner.max_connections
    }

    pub async fn execute(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<ExecuteSummary> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || {
            let mut conn = db.connect()?;
            conn.set_busy_timeout(busy);
            conn.execute(&sql, params)
        })
        .await
    }

    pub async fn fetch_all(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<Vec<AsyncRow>> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || materialize_rows(&db, &sql, params, busy, None))
            .await
    }

    pub async fn fetch_one(&self, sql: impl Into<String>, params: Vec<Value>) -> Result<AsyncRow> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        let rows = self
            .run_blocking(move || materialize_rows(&db, &sql, params, busy, Some(1)))
            .await?;
        match rows.into_iter().next() {
            Some(row) => Ok(row),
            None => Err(Error::new(
                ErrorCode::NotFound,
                "fetch_one: query returned no rows",
            )),
        }
    }

    pub async fn fetch_optional(
        &self,
        sql: impl Into<String>,
        params: Vec<Value>,
    ) -> Result<Option<AsyncRow>> {
        let sql = sql.into();
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        let rows = self
            .run_blocking(move || materialize_rows(&db, &sql, params, busy, Some(1)))
            .await?;
        Ok(rows.into_iter().next())
    }

    pub async fn with_connection<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut redlinedb::Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.inner.db.clone();
        let busy = self.inner.busy_timeout;
        self.run_blocking(move || {
            let mut conn = db.connect()?;
            conn.set_busy_timeout(busy);
            f(&mut conn)
        })
        .await
    }

    pub async fn transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut redlinedb::Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.with_connection(move |conn| {
            conn.begin(BeginMode::Deferred)?;
            match f(conn) {
                Ok(value) => {
                    conn.commit()?;
                    Ok(value)
                }
                Err(e) => {
                    let _ = conn.rollback();
                    Err(e)
                }
            }
        })
        .await
    }

    async fn run_blocking<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = Arc::clone(&self.inner.semaphore)
            .acquire_owned()
            .await
            .map_err(|_| Error::new(ErrorCode::Internal, "pool semaphore closed"))?;
        tokio::task::spawn_blocking(move || {
            let result = f();
            drop(permit);
            result
        })
        .await
        .map_err(join_err)?
    }
}

fn materialize_rows(
    db: &Database,
    sql: &str,
    params: Vec<Value>,
    busy: Duration,
    limit: Option<usize>,
) -> Result<Vec<AsyncRow>> {
    use redlinedb::Step;
    let mut conn = db.connect()?;
    conn.set_busy_timeout(busy);
    let mut stmt = conn.prepare(sql)?;
    stmt.bind_all(params)?;
    let column_count = stmt.column_count();
    let mut names: Vec<String> = Vec::with_capacity(column_count);
    for i in 0..column_count {
        names.push(stmt.column_name(i).to_owned());
    }
    let names_arc: Arc<[String]> = names.into();
    let mut out = Vec::new();
    let cap = limit.unwrap_or(usize::MAX);
    while let Step::Row(row) = stmt.step()? {
        if out.len() >= cap {
            break;
        }
        let mut columns: Vec<Value> = Vec::with_capacity(column_count);
        for i in 0..column_count {
            columns.push(row.get::<Value>(i)?);
        }
        out.push(AsyncRow {
            columns,
            names: Arc::clone(&names_arc),
        });
    }
    Ok(out)
}

fn join_err(err: tokio::task::JoinError) -> Error {
    Error::new(
        ErrorCode::Internal,
        format!("redlinedb-tokio blocking task panicked: {err}"),
    )
}
