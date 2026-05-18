use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use futures_core::future::BoxFuture;
use sqlx::Either;
use sqlx::column::Column;
use sqlx::database::Database;
use sqlx::error::{BoxDynError, Error};
use sqlx::ext::ustr::UStr;
use sqlx::row::Row;
use sqlx::statement::Statement;
use sqlx::transaction::TransactionManager;
use sqlx::type_info::TypeInfo;
use sqlx::types::Type;
use sqlx::value::{Value, ValueRef};

use crate::bridge::{RedlineConnection, join_error, map_redline_error};

/// Marker database used only to register the RedlineDB `Any` driver.
#[derive(Debug)]
pub struct RedlineDb;

pub(crate) const REDLINE_DRIVER: sqlx::any::driver::AnyDriver =
    sqlx::any::driver::AnyDriver::without_migrate::<RedlineDb>();

#[derive(Clone, Debug, PartialEq)]
pub struct RedlineTypeInfo {
    pub kind: RedlineTypeInfoKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedlineTypeInfoKind {
    Null,
    Bool,
    SmallInt,
    Integer,
    BigInt,
    Real,
    Double,
    Text,
    Blob,
}

impl sqlx::type_info::TypeInfo for RedlineTypeInfo {
    fn is_null(&self) -> bool {
        self.kind == RedlineTypeInfoKind::Null
    }

    fn name(&self) -> &str {
        match self.kind {
            RedlineTypeInfoKind::Null => "NULL",
            RedlineTypeInfoKind::Bool => "BOOLEAN",
            RedlineTypeInfoKind::SmallInt => "SMALLINT",
            RedlineTypeInfoKind::Integer => "INTEGER",
            RedlineTypeInfoKind::BigInt => "BIGINT",
            RedlineTypeInfoKind::Real => "REAL",
            RedlineTypeInfoKind::Double => "DOUBLE",
            RedlineTypeInfoKind::Text => "TEXT",
            RedlineTypeInfoKind::Blob => "BLOB",
        }
    }
}

impl Display for RedlineTypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(TypeInfo::name(self))
    }
}

#[derive(Debug, Clone)]
pub struct RedlineColumn {
    pub ordinal: usize,
    pub name: UStr,
    pub type_info: RedlineTypeInfo,
}

impl Column for RedlineColumn {
    type Database = RedlineDb;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &RedlineTypeInfo {
        &self.type_info
    }
}

#[derive(Clone, Debug)]
pub enum RedlineValueKind<'a> {
    Null(RedlineTypeInfoKind),
    Bool(bool),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Real(f32),
    Double(f64),
    Text(Cow<'a, str>),
    Blob(Cow<'a, [u8]>),
}

impl RedlineValueKind<'_> {
    fn type_info(&self) -> RedlineTypeInfo {
        RedlineTypeInfo {
            kind: match self {
                RedlineValueKind::Null(kind) => *kind,
                RedlineValueKind::Bool(_) => RedlineTypeInfoKind::Bool,
                RedlineValueKind::SmallInt(_) => RedlineTypeInfoKind::SmallInt,
                RedlineValueKind::Integer(_) => RedlineTypeInfoKind::Integer,
                RedlineValueKind::BigInt(_) => RedlineTypeInfoKind::BigInt,
                RedlineValueKind::Real(_) => RedlineTypeInfoKind::Real,
                RedlineValueKind::Double(_) => RedlineTypeInfoKind::Double,
                RedlineValueKind::Text(_) => RedlineTypeInfoKind::Text,
                RedlineValueKind::Blob(_) => RedlineTypeInfoKind::Blob,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct RedlineValue {
    pub kind: RedlineValueKind<'static>,
}

#[derive(Clone, Debug)]
pub struct RedlineValueRef<'a> {
    pub kind: RedlineValueKind<'a>,
}

impl Value for RedlineValue {
    type Database = RedlineDb;

    fn as_ref(&self) -> <Self::Database as Database>::ValueRef<'_> {
        RedlineValueRef {
            kind: match &self.kind {
                RedlineValueKind::Null(k) => RedlineValueKind::Null(*k),
                RedlineValueKind::Bool(v) => RedlineValueKind::Bool(*v),
                RedlineValueKind::SmallInt(v) => RedlineValueKind::SmallInt(*v),
                RedlineValueKind::Integer(v) => RedlineValueKind::Integer(*v),
                RedlineValueKind::BigInt(v) => RedlineValueKind::BigInt(*v),
                RedlineValueKind::Real(v) => RedlineValueKind::Real(*v),
                RedlineValueKind::Double(v) => RedlineValueKind::Double(*v),
                RedlineValueKind::Text(v) => RedlineValueKind::Text(Cow::Borrowed(v)),
                RedlineValueKind::Blob(v) => RedlineValueKind::Blob(Cow::Borrowed(v)),
            },
        }
    }

    fn type_info(&self) -> std::borrow::Cow<'_, <Self::Database as Database>::TypeInfo> {
        Cow::Owned(self.kind.type_info())
    }

    fn is_null(&self) -> bool {
        matches!(self.kind, RedlineValueKind::Null(_))
    }
}

impl<'a> ValueRef<'a> for RedlineValueRef<'a> {
    type Database = RedlineDb;

    fn to_owned(&self) -> <Self::Database as Database>::Value {
        RedlineValue {
            kind: match &self.kind {
                RedlineValueKind::Null(k) => RedlineValueKind::Null(*k),
                RedlineValueKind::Bool(v) => RedlineValueKind::Bool(*v),
                RedlineValueKind::SmallInt(v) => RedlineValueKind::SmallInt(*v),
                RedlineValueKind::Integer(v) => RedlineValueKind::Integer(*v),
                RedlineValueKind::BigInt(v) => RedlineValueKind::BigInt(*v),
                RedlineValueKind::Real(v) => RedlineValueKind::Real(*v),
                RedlineValueKind::Double(v) => RedlineValueKind::Double(*v),
                RedlineValueKind::Text(v) => RedlineValueKind::Text(Cow::Owned(v.to_string())),
                RedlineValueKind::Blob(v) => RedlineValueKind::Blob(Cow::Owned(v.to_vec())),
            },
        }
    }

    fn type_info(&self) -> std::borrow::Cow<'_, <Self::Database as Database>::TypeInfo> {
        Cow::Owned(self.kind.type_info())
    }

    fn is_null(&self) -> bool {
        matches!(self.kind, RedlineValueKind::Null(_))
    }
}

#[derive(Clone, Debug, Default)]
pub struct RedlineQueryResult {
    pub rows_affected: u64,
    pub last_insert_id: Option<i64>,
}

impl Extend<RedlineQueryResult> for RedlineQueryResult {
    fn extend<T: IntoIterator<Item = RedlineQueryResult>>(&mut self, iter: T) {
        for result in iter {
            self.rows_affected += result.rows_affected;
            self.last_insert_id = result.last_insert_id;
        }
    }
}

#[derive(Clone, Debug)]
pub struct RedlineArguments<'q> {
    pub values: RedlineArgumentBuffer<'q>,
}

#[derive(Clone, Debug)]
pub struct RedlineArgumentBuffer<'q>(pub Vec<RedlineValueKind<'q>>);

impl<'q> Default for RedlineArguments<'q> {
    fn default() -> Self {
        Self {
            values: RedlineArgumentBuffer(Vec::new()),
        }
    }
}

impl<'q> sqlx::arguments::Arguments<'q> for RedlineArguments<'q> {
    type Database = RedlineDb;

    fn reserve(&mut self, additional: usize, _size: usize) {
        self.values.0.reserve(additional);
    }

    fn add<T>(&mut self, _value: T) -> Result<(), BoxDynError>
    where
        T: 'q + sqlx::encode::Encode<'q, Self::Database> + Type<Self::Database>,
    {
        Err("RedlineDb sqlx arguments are not used by the redline:// bridge".into())
    }

    fn len(&self) -> usize {
        self.values.0.len()
    }
}

#[derive(Clone, Debug)]
pub struct RedlineStatement<'q> {
    pub sql: Cow<'q, str>,
    pub parameters: Option<Either<Vec<RedlineTypeInfo>, usize>>,
    pub column_names: Arc<HashMap<UStr, usize>>,
    pub columns: Vec<RedlineColumn>,
}

impl<'q> Statement<'q> for RedlineStatement<'q> {
    type Database = RedlineDb;

    fn to_owned(&self) -> RedlineStatement<'static> {
        RedlineStatement {
            sql: Cow::Owned(self.sql.clone().into_owned()),
            parameters: self.parameters.clone(),
            column_names: self.column_names.clone(),
            columns: self.columns.clone(),
        }
    }

    fn sql(&self) -> &str {
        &self.sql
    }

    fn parameters(&self) -> Option<Either<&[RedlineTypeInfo], usize>> {
        match &self.parameters {
            Some(Either::Left(params)) => Some(Either::Left(params)),
            Some(Either::Right(count)) => Some(Either::Right(*count)),
            None => None,
        }
    }

    fn columns(&self) -> &[RedlineColumn] {
        &self.columns
    }

    sqlx::impl_statement_query!(RedlineArguments<'_>);
}

impl sqlx::column::ColumnIndex<RedlineStatement<'_>> for usize {
    fn index(&self, _statement: &RedlineStatement<'_>) -> Result<usize, Error> {
        Ok(*self)
    }
}

impl<'i> sqlx::column::ColumnIndex<RedlineStatement<'_>> for &'i str {
    fn index(&self, statement: &RedlineStatement<'_>) -> Result<usize, Error> {
        statement
            .column_names
            .get(*self)
            .copied()
            .ok_or_else(|| Error::ColumnNotFound((*self).to_string()))
    }
}

#[derive(Clone, Debug)]
pub struct RedlineRow {
    pub columns: Vec<RedlineColumn>,
    pub values: Vec<RedlineValue>,
}

impl Row for RedlineRow {
    type Database = RedlineDb;

    fn columns(&self) -> &[RedlineColumn] {
        &self.columns
    }

    fn try_get_raw<I>(&self, index: I) -> Result<<Self::Database as Database>::ValueRef<'_>, Error>
    where
        I: sqlx::column::ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        Ok(self
            .values
            .get(index)
            .ok_or_else(|| Error::ColumnIndexOutOfBounds {
                index,
                len: self.columns.len(),
            })?
            .as_ref())
    }

    fn try_get<'r, T, I>(&'r self, _index: I) -> Result<T, Error>
    where
        I: sqlx::column::ColumnIndex<Self>,
        T: sqlx::decode::Decode<'r, Self::Database> + Type<Self::Database>,
    {
        Err(Error::Protocol(
            "RedlineDb row decoding is not used by the bridge".into(),
        ))
    }
}

impl<'i> sqlx::column::ColumnIndex<RedlineRow> for &'i str {
    fn index(&self, row: &RedlineRow) -> Result<usize, Error> {
        row.columns
            .iter()
            .position(|column| <str as AsRef<str>>::as_ref(&column.name) == *self)
            .ok_or_else(|| Error::ColumnNotFound((*self).to_string()))
    }
}

impl RedlineDb {
    pub(crate) const NAME: &'static str = "RedlineDB";
    pub(crate) const URL_SCHEMES: &'static [&'static str] = &["redline"];
}

impl Database for RedlineDb {
    const NAME: &'static str = RedlineDb::NAME;
    const URL_SCHEMES: &'static [&'static str] = RedlineDb::URL_SCHEMES;

    type Connection = RedlineConnection;
    type TransactionManager = RedlineTransactionManager;
    type Row = RedlineRow;
    type QueryResult = RedlineQueryResult;
    type Column = RedlineColumn;
    type TypeInfo = RedlineTypeInfo;
    type Value = RedlineValue;
    type ValueRef<'r> = RedlineValueRef<'r>;
    type Arguments<'q> = RedlineArguments<'q>;
    type ArgumentBuffer<'q> = RedlineArgumentBuffer<'q>;
    type Statement<'q> = RedlineStatement<'q>;
}

pub struct RedlineTransactionManager;

impl TransactionManager for RedlineTransactionManager {
    type Database = RedlineDb;

    fn begin<'conn>(
        conn: &'conn mut RedlineConnection,
        statement: Option<Cow<'static, str>>,
    ) -> BoxFuture<'conn, Result<(), Error>> {
        let state = Arc::clone(&conn.state);
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

    fn commit(conn: &mut RedlineConnection) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&conn.state);
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

    fn rollback(conn: &mut RedlineConnection) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&conn.state);
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

    fn start_rollback(conn: &mut RedlineConnection) {
        let state = Arc::clone(&conn.state);
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(mut guard) = state.lock() {
                let _ = guard.conn.rollback();
            }
        });
    }

    fn get_transaction_depth(conn: &<Self::Database as Database>::Connection) -> usize {
        match conn.state.lock() {
            Ok(guard) => usize::from(guard.conn.in_transaction()),
            Err(_) => 0,
        }
    }
}
