use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt, stream};
use sqlx::Either;
use sqlx::HashMap;
use sqlx::any::{
    self, Any, AnyColumn, AnyConnectOptions, AnyConnectionBackend, AnyQueryResult, AnyRow,
    AnyStatement, AnyTypeInfo, AnyTypeInfoKind, AnyValue, AnyValueKind,
};
use sqlx::connection::{ConnectOptions, Connection};
use sqlx::describe::Describe;
use sqlx::error::Error;
use sqlx::ext::ustr::UStr;
use sqlx::transaction::Transaction;
use url::Url;

use crate::dummy::RedlineDb;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct ConnectionState {
    #[allow(dead_code)]
    pub(crate) db: redlinedb::Database,
    #[allow(dead_code)]
    pub(crate) conn: redlinedb::Connection,
}

impl std::fmt::Debug for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionState").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct RedlineConnectOptions {
    database_url: Url,
    location: RedlineLocation,
    busy_timeout: Duration,
}

#[derive(Clone, Debug)]
enum RedlineLocation {
    File(PathBuf),
    InMemory,
}

#[derive(Debug)]
pub struct RedlineConnection {
    pub(crate) state: Arc<Mutex<ConnectionState>>,
}

impl RedlineConnection {
    fn with_state<T>(
        state: Arc<Mutex<ConnectionState>>,
        f: impl FnOnce(&mut redlinedb::Connection) -> Result<T, Error> + Send + 'static,
    ) -> BoxFuture<'static, Result<T, Error>>
    where
        T: Send + 'static,
    {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut guard = state
                    .lock()
                    .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
                f(&mut guard.conn)
            })
            .await
            .map_err(join_error)?
        })
    }

    fn connect_sync(options: RedlineConnectOptions) -> Result<Self, Error> {
        let db = match options.location {
            RedlineLocation::File(path) => {
                redlinedb::Database::create(path).map_err(map_redline_error)?
            }
            RedlineLocation::InMemory => {
                redlinedb::Database::create_in_memory(redlinedb::OpenOptions::default())
                    .map_err(map_redline_error)?
            }
        };
        let conn = db.connect().map_err(map_redline_error)?;

        let mut conn = conn;
        conn.set_busy_timeout(options.busy_timeout);

        Ok(Self {
            state: Arc::new(Mutex::new(ConnectionState { db, conn })),
        })
    }
}

impl ConnectOptions for RedlineConnectOptions {
    type Connection = RedlineConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        if url.scheme() != "redline" {
            return Err(Error::Configuration(
                format!("unsupported URL scheme {url:?} for RedlineDB").into(),
            ));
        }

        let location = parse_location(url)?;

        Ok(Self {
            database_url: url.clone(),
            location,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        })
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, Error>> {
        let options = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || RedlineConnection::connect_sync(options))
                .await
                .map_err(join_error)?
        })
    }

    fn log_statements(self, _level: log::LevelFilter) -> Self {
        self
    }

    fn log_slow_statements(self, _level: log::LevelFilter, _duration: Duration) -> Self {
        self
    }

    fn to_url_lossy(&self) -> Url {
        self.database_url.clone()
    }
}

impl std::str::FromStr for RedlineConnectOptions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s).map_err(Error::config)?;
        Self::from_url(&url)
    }
}

impl TryFrom<&AnyConnectOptions> for RedlineConnectOptions {
    type Error = Error;

    fn try_from(options: &AnyConnectOptions) -> Result<Self, Self::Error> {
        Self::from_url(&options.database_url)
    }
}

impl Connection for RedlineConnection {
    type Database = RedlineDb;
    type Options = RedlineConnectOptions;

    fn close(self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn close_hard(self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        Self::with_state(state, |_conn| Ok(()))
    }

    fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, None)
    }

    fn begin_with(
        &mut self,
        statement: impl Into<Cow<'static, str>>,
    ) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, Some(statement.into()))
    }

    fn cached_statements_size(&self) -> usize {
        0
    }

    fn clear_cached_statements(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }

    fn shrink_buffers(&mut self) {}

    fn flush(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }

    fn should_flush(&self) -> bool {
        false
    }
}

pub(crate) enum QueryOutcome {
    Rows(Vec<AnyRow>),
    Result(AnyQueryResult),
}

impl QueryOutcome {
    fn into_stream(
        self,
    ) -> impl futures_core::stream::Stream<Item = Result<Either<AnyQueryResult, AnyRow>, Error>>
    {
        match self {
            QueryOutcome::Rows(rows) => {
                stream::iter(rows.into_iter().map(|row| Ok(Either::Right(row)))).boxed()
            }
            QueryOutcome::Result(result) => stream::iter([Ok(Either::Left(result))]).boxed(),
        }
    }
}

impl AnyConnectionBackend for RedlineConnection {
    fn name(&self) -> &str {
        RedlineDb::NAME
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn close_hard(self: Box<Self>) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        RedlineConnection::with_state(state, |_conn| Ok(()))
    }

    fn begin(&mut self, statement: Option<Cow<'static, str>>) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut guard = state
                    .lock()
                    .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
                let mode = match statement.as_deref() {
                    Some(s) if s.eq_ignore_ascii_case("begin immediate") => {
                        redlinedb::BeginMode::Immediate
                    }
                    Some(s) if s.eq_ignore_ascii_case("begin exclusive") => {
                        redlinedb::BeginMode::Exclusive
                    }
                    _ => redlinedb::BeginMode::Deferred,
                };
                guard.conn.begin(mode).map_err(map_redline_error)?;
                Ok(())
            })
            .await
            .map_err(join_error)?
        })
    }

    fn commit(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut guard = state
                    .lock()
                    .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
                guard.conn.commit().map_err(map_redline_error)?;
                Ok(())
            })
            .await
            .map_err(join_error)?
        })
    }

    fn rollback(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut guard = state
                    .lock()
                    .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
                guard.conn.rollback().map_err(map_redline_error)?;
                Ok(())
            })
            .await
            .map_err(join_error)?
        })
    }

    fn start_rollback(&mut self) {
        let state = Arc::clone(&self.state);
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(mut guard) = state.lock() {
                let _ = guard.conn.rollback();
            }
        });
    }

    fn get_transaction_depth(&self) -> usize {
        match self.state.lock() {
            Ok(guard) => usize::from(guard.conn.in_transaction()),
            Err(_) => 0,
        }
    }

    fn shrink_buffers(&mut self) {}

    fn flush(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }

    fn should_flush(&self) -> bool {
        false
    }

    fn fetch_many<'q>(
        &'q mut self,
        query: &'q str,
        _persistent: bool,
        arguments: Option<any::AnyArguments<'q>>,
    ) -> BoxStream<'q, Result<Either<AnyQueryResult, AnyRow>, Error>> {
        let state = Arc::clone(&self.state);
        let sql = query.to_owned();
        let args = arguments.map(any_arguments_to_redline);

        Box::pin(
            stream::once(async move {
                let outcome = execute_query(state, sql, args.unwrap_or_default()).await?;
                Ok::<_, Error>(outcome.into_stream())
            })
            .try_flatten(),
        )
    }

    fn fetch_optional<'q>(
        &'q mut self,
        query: &'q str,
        _persistent: bool,
        arguments: Option<any::AnyArguments<'q>>,
    ) -> BoxFuture<'q, Result<Option<AnyRow>, Error>> {
        let state = Arc::clone(&self.state);
        let sql = query.to_owned();
        let args = arguments.map(any_arguments_to_redline);

        Box::pin(async move {
            match execute_query(state, sql, args.unwrap_or_default()).await? {
                QueryOutcome::Rows(mut rows) => Ok(rows.drain(..).next()),
                QueryOutcome::Result(_) => Ok(None),
            }
        })
    }

    fn prepare_with<'c, 'q: 'c>(
        &'c mut self,
        sql: &'q str,
        _parameters: &[AnyTypeInfo],
    ) -> BoxFuture<'c, Result<AnyStatement<'q>, Error>> {
        let state = Arc::clone(&self.state);
        let sql_owned = sql.to_owned();

        Box::pin(async move {
            let description = describe_statement(state, sql_owned.clone()).await?;
            Ok(AnyStatement {
                sql: Cow::Owned(sql_owned),
                parameters: description.parameters,
                column_names: description.column_names,
                columns: description.columns,
            })
        })
    }

    fn describe<'q>(&'q mut self, sql: &'q str) -> BoxFuture<'q, Result<Describe<Any>, Error>> {
        let state = Arc::clone(&self.state);
        let sql_owned = sql.to_owned();

        Box::pin(async move {
            let statement = describe_statement(state, sql_owned).await?;
            let column_count = statement.columns.len();
            Ok(Describe {
                columns: statement.columns,
                parameters: statement.parameters,
                nullable: vec![None; column_count],
            })
        })
    }
}

fn parse_location(url: &Url) -> Result<RedlineLocation, Error> {
    let mut raw = String::new();
    if let Some(host) = url.host_str() {
        raw.push_str(host);
    }
    raw.push_str(url.path());

    if raw.is_empty() || raw == "/" || raw == "/:memory:" || raw == ":memory:" {
        return Ok(RedlineLocation::InMemory);
    }

    Ok(RedlineLocation::File(PathBuf::from(raw)))
}

fn any_arguments_to_redline(arguments: any::AnyArguments<'_>) -> Vec<redlinedb::Value> {
    arguments
        .values
        .0
        .into_iter()
        .map(|kind| match kind {
            AnyValueKind::Null(_) => redlinedb::Value::Null,
            AnyValueKind::Bool(value) => redlinedb::Value::from(value),
            AnyValueKind::SmallInt(value) => redlinedb::Value::from(value),
            AnyValueKind::Integer(value) => redlinedb::Value::from(i64::from(value)),
            AnyValueKind::BigInt(value) => redlinedb::Value::from(value),
            AnyValueKind::Real(value) => redlinedb::Value::from(f64::from(value)),
            AnyValueKind::Double(value) => redlinedb::Value::from(value),
            AnyValueKind::Text(value) => redlinedb::Value::from(value.into_owned()),
            AnyValueKind::Blob(value) => redlinedb::Value::from(value.into_owned()),
            _ => redlinedb::Value::Null,
        })
        .collect()
}

async fn execute_query(
    state: Arc<Mutex<ConnectionState>>,
    sql: String,
    args: Vec<redlinedb::Value>,
) -> Result<QueryOutcome, Error> {
    tokio::task::spawn_blocking(move || {
        let mut guard = state
            .lock()
            .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
        let mut stmt = guard.conn.prepare(&sql).map_err(map_redline_error)?;

        for (index, value) in args.into_iter().enumerate() {
            stmt.bind_value(index + 1, value)
                .map_err(map_redline_error)?;
        }

        let column_count = stmt.column_count();
        let column_names = Arc::new(
            (0..column_count)
                .map(|index| (UStr::from(stmt.column_name(index).to_owned()), index))
                .collect::<HashMap<_, _>>(),
        );
        let column_names_vec = (0..column_count)
            .map(|index| UStr::from(stmt.column_name(index).to_owned()))
            .collect::<Vec<_>>();

        if column_count > 0 {
            let mut rows = Vec::new();
            loop {
                match stmt.step().map_err(map_redline_error)? {
                    redlinedb::Step::Row(row) => {
                        rows.push(build_any_row(&column_names, &column_names_vec, &row)?);
                    }
                    redlinedb::Step::Done => break,
                }
            }
            Ok(QueryOutcome::Rows(rows))
        } else {
            while let redlinedb::Step::Row(_) = stmt.step().map_err(map_redline_error)? {}
            Ok(QueryOutcome::Result(AnyQueryResult {
                rows_affected: stmt.affected_rows() as u64,
                last_insert_id: guard.conn.last_insert_rowid(),
            }))
        }
    })
    .await
    .map_err(join_error)?
}

struct StatementDescription {
    parameters: Option<Either<Vec<AnyTypeInfo>, usize>>,
    column_names: Arc<HashMap<UStr, usize>>,
    columns: Vec<AnyColumn>,
}

async fn describe_statement(
    state: Arc<Mutex<ConnectionState>>,
    sql: String,
) -> Result<StatementDescription, Error> {
    tokio::task::spawn_blocking(move || {
        let mut guard = state
            .lock()
            .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
        let stmt = guard.conn.prepare(&sql).map_err(map_redline_error)?;
        let column_count = stmt.column_count();
        let column_names = Arc::new(
            (0..column_count)
                .map(|index| (UStr::from(stmt.column_name(index).to_owned()), index))
                .collect::<HashMap<_, _>>(),
        );
        let column_names_vec = (0..column_count)
            .map(|index| UStr::from(stmt.column_name(index).to_owned()))
            .collect::<Vec<_>>();

        let columns = (0..column_count)
            .map(|index| AnyColumn {
                ordinal: index,
                name: column_names_vec[index].clone(),
                type_info: AnyTypeInfo {
                    kind: AnyTypeInfoKind::Null,
                },
            })
            .collect();

        Ok(StatementDescription {
            parameters: Some(Either::Right(stmt.parameter_count())),
            column_names,
            columns,
        })
    })
    .await
    .map_err(join_error)?
}

fn build_any_row(
    column_names: &Arc<HashMap<UStr, usize>>,
    column_names_vec: &[UStr],
    row: &redlinedb::Row<'_>,
) -> Result<AnyRow, Error> {
    let column_count = column_names_vec.len();
    let mut columns = Vec::with_capacity(column_count);
    let mut values = Vec::with_capacity(column_count);

    for index in 0..column_count {
        let value: redlinedb::Value = row.get(index).map_err(map_redline_error)?;
        let (any_kind, type_info_kind) = any_value_from_redline(value);
        columns.push(AnyColumn {
            ordinal: index,
            name: column_names_vec[index].clone(),
            type_info: AnyTypeInfo {
                kind: type_info_kind,
            },
        });
        values.push(any_kind);
    }

    Ok(AnyRow {
        column_names: column_names.clone(),
        columns,
        values,
    })
}

fn any_value_from_redline(value: redlinedb::Value) -> (AnyValue, AnyTypeInfoKind) {
    match value {
        redlinedb::Value::Null => (
            AnyValue {
                kind: AnyValueKind::Null(AnyTypeInfoKind::Null),
            },
            AnyTypeInfoKind::Null,
        ),
        redlinedb::Value::Integer(value) => (
            AnyValue {
                kind: AnyValueKind::BigInt(value),
            },
            AnyTypeInfoKind::BigInt,
        ),
        redlinedb::Value::Real(value) => (
            AnyValue {
                kind: AnyValueKind::Double(value),
            },
            AnyTypeInfoKind::Double,
        ),
        redlinedb::Value::Text(value) => (
            AnyValue {
                kind: AnyValueKind::Text(Cow::Owned(value.to_string())),
            },
            AnyTypeInfoKind::Text,
        ),
        redlinedb::Value::Blob(value) => (
            AnyValue {
                kind: AnyValueKind::Blob(Cow::Owned(value.to_vec())),
            },
            AnyTypeInfoKind::Blob,
        ),
    }
}

pub(crate) fn map_redline_error(err: redlinedb::Error) -> Error {
    Error::AnyDriverError(err.into())
}

pub(crate) fn join_error(err: tokio::task::JoinError) -> Error {
    if err.is_panic() {
        Error::WorkerCrashed
    } else {
        Error::Protocol(format!("redline blocking task failed: {err}"))
    }
}

pub(crate) fn install_redline_driver_once() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        any::driver::install_drivers(&[crate::dummy::REDLINE_DRIVER])
            .expect("redline driver already installed")
    });
}
