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

use crate::driver::RedlineDb;

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const REDLINE_URL_SCHEMES: &[&str] = &["redline", "redlinedb"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RedlineUrlMode {
    Rwc,
    Ro,
}

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
    pub(crate) database_url: Url,
    pub(crate) location: RedlineLocation,
    pub(crate) open_options: redlinedb::OpenOptions,
    pub(crate) busy_timeout: Duration,
}

#[derive(Clone, Debug)]
pub(crate) enum RedlineLocation {
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
        let RedlineConnectOptions {
            location,
            open_options,
            busy_timeout,
            ..
        } = options;

        let db = match location {
            RedlineLocation::File(path) => {
                redlinedb::Database::open_with_options(path, open_options.clone())
                    .map_err(map_redline_error)?
            }
            RedlineLocation::InMemory => {
                redlinedb::Database::create_in_memory(open_options.clone())
                    .map_err(map_redline_error)?
            }
        };
        let conn = db.connect().map_err(map_redline_error)?;

        let mut conn = conn;
        conn.set_busy_timeout(busy_timeout);

        Ok(Self {
            state: Arc::new(Mutex::new(ConnectionState { db, conn })),
        })
    }
}

impl ConnectOptions for RedlineConnectOptions {
    type Connection = RedlineConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        if !REDLINE_URL_SCHEMES.contains(&url.scheme()) {
            return Err(Error::Configuration(
                format!(
                    "unsupported URL scheme {:?} for RedlineDB; expected redline:// or redlinedb://",
                    url.scheme()
                )
                .into(),
            ));
        }

        let mode = parse_mode(url)?;
        let location = parse_location(url)?;
        if matches!(location, RedlineLocation::InMemory) && mode == RedlineUrlMode::Ro {
            // HLT-022-AUTHZ-ISOLATION-GAP negative proof for the attach boundary:
            // crates/redlinedb-sqlx/tests/attach_mode.rs::mode_ro_attaches_to_live_database_and_blocks_writes
            // proves the non-owner attach path can read live rows, rejects writes,
            // and a second rwc opener still hits the owner lock.
            return Err(Error::Configuration(
                "read-only attach mode is only supported for file-backed RedlineDB URLs".into(),
            ));
        }

        let mut open_options = redlinedb::OpenOptions::default();
        match mode {
            RedlineUrlMode::Rwc => {
                open_options.create = true;
                open_options.read_only = false;
                open_options.process_owner_lock = true;
            }
            RedlineUrlMode::Ro => {
                open_options.create = false;
                open_options.read_only = true;
                open_options.process_owner_lock = false;
            }
        }

        Ok(Self {
            database_url: url.clone(),
            location,
            open_options,
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
                let args = match args {
                    Some(args) => args,
                    None => Vec::new(),
                };
                let outcome = execute_query(state, sql, args).await?;
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
            let args = match args {
                Some(args) => args,
                None => Vec::new(),
            };
            match execute_query(state, sql, args).await? {
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

fn parse_mode(url: &Url) -> Result<RedlineUrlMode, Error> {
    let mut mode = None;

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "mode" => {
                if mode.is_some() {
                    return Err(Error::Configuration(
                        "duplicate mode query parameter in RedlineDB URL".into(),
                    ));
                }

                mode = Some(match value.as_ref() {
                    "rwc" => RedlineUrlMode::Rwc,
                    "ro" => RedlineUrlMode::Ro,
                    other => {
                        return Err(Error::Configuration(
                            format!(
                                "unsupported RedlineDB mode {:?}; expected mode=rwc or mode=ro",
                                other
                            )
                            .into(),
                        ));
                    }
                });
            }
            other => {
                return Err(Error::Configuration(
                    format!(
                        "unsupported RedlineDB URL query parameter {:?}; expected mode=rwc or mode=ro",
                        other
                    )
                    .into(),
                ));
            }
        }
    }

    Ok(mode.unwrap_or(RedlineUrlMode::Rwc))
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
        let (sql, args) = if should_inline_parameters_for_prepare_time_sql(&sql, &args) {
            (inline_qmark_parameters(&sql, &args)?, Vec::new())
        } else {
            (sql, args)
        };
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

fn should_inline_parameters_for_prepare_time_sql(sql: &str, args: &[redlinedb::Value]) -> bool {
    !args.is_empty() && sql.trim_start().to_ascii_lowercase().starts_with("with")
}

fn inline_qmark_parameters(sql: &str, args: &[redlinedb::Value]) -> Result<String, Error> {
    let mut out = String::with_capacity(sql.len() + args.len() * 8);
    let mut arg_index = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double_quote => {
                out.push(ch);
                if in_single_quote && chars.peek() == Some(&'\'') {
                    out.push(chars.next().expect("peeked escaped quote"));
                } else {
                    in_single_quote = !in_single_quote;
                }
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                out.push(ch);
            }
            '?' if !in_single_quote && !in_double_quote => {
                let value = args.get(arg_index).ok_or_else(|| {
                    Error::Protocol("not enough bind values for parameterized WITH query".into())
                })?;
                out.push_str(&redline_value_literal(value));
                arg_index += 1;
            }
            _ => out.push(ch),
        }
    }

    if arg_index != args.len() {
        return Err(Error::Protocol(
            "too many bind values for parameterized WITH query".into(),
        ));
    }
    Ok(out)
}

fn redline_value_literal(value: &redlinedb::Value) -> String {
    match value {
        redlinedb::Value::Null => "NULL".to_owned(),
        redlinedb::Value::Integer(value) => value.to_string(),
        redlinedb::Value::Real(value) if value.is_finite() => value.to_string(),
        redlinedb::Value::Real(_) => "NULL".to_owned(),
        redlinedb::Value::Text(value) => {
            let escaped = value.replace('\'', "''");
            format!("'{escaped}'")
        }
        redlinedb::Value::Blob(value) => {
            let mut out = String::with_capacity(value.len() * 2 + 3);
            out.push_str("x'");
            for byte in value.iter() {
                use std::fmt::Write as _;
                let _ = write!(&mut out, "{byte:02x}");
            }
            out.push('\'');
            out
        }
    }
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
        any::driver::install_drivers(&[crate::driver::REDLINE_DRIVER])
            .expect("redline driver already installed")
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install_default_drivers;
    use sqlx::{any::AnyPoolOptions, row::Row};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    const HELPER_ROLE_ENV: &str = "REDLINEDB_SQLX_ATTACH_ROLE";
    const HELPER_DB_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_DB_PATH";
    const HELPER_READY_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_READY_PATH";
    const HELPER_HOLD_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_HOLD_PATH";
    const HELPER_TEST_NAME: &str = "bridge::tests::owner_process_holds_database_open";

    #[test]
    fn mode_ro_disables_owner_lock_and_creation() {
        let url = Url::parse("redline:///tmp/attach.redlineDB?mode=ro").expect("url");
        let opts = RedlineConnectOptions::from_url(&url).expect("connect options");

        assert!(matches!(opts.location, RedlineLocation::File(_)));
        assert!(!opts.open_options.create);
        assert!(opts.open_options.read_only);
        assert!(!opts.open_options.process_owner_lock);
    }

    #[test]
    fn ro_is_rejected_for_in_memory_urls() {
        let url = Url::parse("redline:///:memory:?mode=ro").expect("url");
        let err = RedlineConnectOptions::from_url(&url).expect_err("reject memory ro");

        assert!(
            err.to_string()
                .contains("read-only attach mode is only supported for file-backed"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn mode_ro_attaches_to_live_database_and_blocks_writes() {
        install_default_drivers();

        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("bridge-attach-mode-ro.db");
        let ready_path = tempdir.path().join("owner.ready");
        let hold_path = tempdir.path().join("owner.hold");
        fs::write(&hold_path, b"hold").expect("create hold file");

        let mut child = spawn_helper(HELPER_TEST_NAME, &db_path, &ready_path, &hold_path);

        wait_for_marker(
            &mut child,
            &ready_path,
            Instant::now() + Duration::from_secs(10),
        );

        let ro_url = format!("redline://{}?mode=ro", db_path.display());
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect(&ro_url)
            .await
            .expect("connect ro");

        let row = sqlx::query::query("SELECT name FROM items WHERE id = ?")
            .bind(1_i64)
            .fetch_one(&pool)
            .await
            .expect("read live row");
        assert_eq!(row.try_get::<String, _>(0).unwrap(), "Ada");

        let write_err = sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
            .bind(2_i64)
            .bind("Grace")
            .execute(&pool)
            .await
            .expect_err("read-only insert should fail");
        assert!(
            write_err.to_string().contains("read-only"),
            "unexpected read-only error: {write_err}"
        );

        drop(pool);

        let rwc_url = format!("redline://{}?mode=rwc", db_path.display());
        let owner_err = AnyPoolOptions::new()
            .max_connections(1)
            .connect(&rwc_url)
            .await
            .expect_err("second writer should hit owner lock");
        assert!(
            owner_err.to_string().contains("database already open"),
            "unexpected owner-lock error: {owner_err}"
        );

        fs::remove_file(&hold_path).expect("release owner helper");
        reap_child(
            &mut child,
            Instant::now() + Duration::from_secs(10),
            "owner_process_holds_database_open",
        );
    }

    #[tokio::test]
    async fn ro_attach_is_rejected_for_in_memory_urls() {
        install_default_drivers();

        let err = AnyPoolOptions::new()
            .max_connections(1)
            .connect("redline:///:memory:?mode=ro")
            .await
            .expect_err("read-only memory attach should fail");
        assert!(
            err.to_string()
                .contains("read-only attach mode is only supported for file-backed"),
            "unexpected memory attach error: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    #[ignore]
    async fn owner_process_holds_database_open() {
        match std::env::var(HELPER_ROLE_ENV) {
            Ok(role) if role == "owner" => {}
            _ => panic!("helper test must run with REDLINEDB_SQLX_ATTACH_ROLE=owner"),
        }

        install_default_drivers();

        let db_path = path_env(HELPER_DB_PATH_ENV);
        let ready_path = path_env(HELPER_READY_PATH_ENV);
        let hold_path = path_env(HELPER_HOLD_PATH_ENV);
        let url = format!("redline://{}?mode=rwc", db_path.display());

        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("owner connect");

        sqlx::query::query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await
            .expect("create items");

        sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
            .bind(1_i64)
            .bind("Ada")
            .execute(&pool)
            .await
            .expect("seed live row");

        fs::write(&ready_path, b"ready").expect("write ready marker");

        let deadline = Instant::now() + Duration::from_secs(10);
        while hold_path.exists() {
            if Instant::now() > deadline {
                panic!("timed out waiting for parent to release hold file");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn spawn_helper(test_name: &str, db_path: &Path, ready_path: &Path, hold_path: &Path) -> Child {
        let current_exe = std::env::current_exe().expect("current exe");
        Command::new(current_exe)
            .args(["--exact", test_name, "--ignored", "--nocapture"])
            .env(HELPER_ROLE_ENV, "owner")
            .env(HELPER_DB_PATH_ENV, db_path)
            .env(HELPER_READY_PATH_ENV, ready_path)
            .env(HELPER_HOLD_PATH_ENV, hold_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn helper")
    }

    fn wait_for_marker(child: &mut Child, path: &Path, deadline: Instant) {
        loop {
            if path.exists() {
                return;
            }
            if let Some(status) = child.try_wait().expect("wait helper") {
                panic!("helper exited before writing marker: {status}");
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let status = child.wait().expect("wait helper after kill");
                panic!("timed out waiting for helper marker: {status}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn reap_child(child: &mut Child, deadline: Instant, label: &str) {
        loop {
            if let Some(status) = child.try_wait().expect("wait helper") {
                assert!(status.success(), "{label} failed: {status}");
                return;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let status = child.wait().expect("wait helper after kill");
                panic!("{label} did not exit before deadline: {status}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn path_env(name: &str) -> PathBuf {
        PathBuf::from(std::env::var_os(name).expect("missing helper path env"))
    }
}
