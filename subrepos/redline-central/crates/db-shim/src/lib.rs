//! Backend-neutral database contract for the governed Jain/Redline operation subset.
//!
//! Exactly one adapter is selected at compile time. Applications use the owned [`Value`],
//! [`Statement`], [`SqlPart`], [`Db`], and [`Error`] types and change only their dependency feature
//! and DSN when selecting a provider. SQLite and Postgres are oracle adapters; Redline is the
//! production default. This crate does not claim arbitrary SQL-dialect parity beyond
//! [`corpus::VERSION`].

use std::env;

#[cfg(feature = "oracle-postgres")]
mod adapter_postgres;
#[cfg(feature = "backend-redline")]
mod adapter_redline;
#[cfg(feature = "oracle-sqlite")]
mod adapter_sqlite;
mod contract;
pub mod corpus;

pub(crate) use contract::{validate_identifier, PlaceholderStyle};
pub use contract::{
    Backend, Capabilities, DbError, Error, Identifier, Result, SqlPart, Statement, TransactionMode,
    Value, ValueType, GOVERNED_CAPABILITIES,
};

#[cfg(not(any(
    feature = "backend-redline",
    feature = "oracle-postgres",
    feature = "oracle-sqlite"
)))]
compile_error!("select exactly one db-shim adapter feature");

#[cfg(any(
    all(feature = "backend-redline", feature = "oracle-postgres"),
    all(feature = "backend-redline", feature = "oracle-sqlite"),
    all(feature = "oracle-postgres", feature = "oracle-sqlite")
))]
compile_error!("db-shim adapter features are mutually exclusive");

/// Backend-neutral database handle selected once at the composition root.
pub struct Db {
    backend: Box<dyn Backend>,
    namespace: String,
}

impl Db {
    /// Open the compile-time selected adapter from neutral DSN and namespace data.
    pub fn open(dsn: &str, namespace: &str) -> Result<Self> {
        validate_identifier(namespace, true)?;
        Ok(Self {
            backend: open_selected(dsn)?,
            namespace: namespace.to_owned(),
        })
    }

    /// Open from neutral configuration: `DB_DSN` and `DB_NAMESPACE`.
    pub fn from_env() -> Result<Self> {
        let dsn = env::var("DB_DSN")
            .map_err(|_| Error::Config("DB_DSN must be set for the selected adapter".to_owned()))?;
        let namespace = env::var("DB_NAMESPACE").unwrap_or_default();
        Self::open(&dsn, &namespace)
    }

    pub fn capabilities(&self) -> Capabilities {
        self.backend.capabilities()
    }

    /// Construct a validated namespaced table identifier without rewriting SQL text.
    pub fn table(&self, table: &str) -> Result<Identifier> {
        validate_identifier(table, false)?;
        let identifier = if self.namespace.is_empty() {
            table.to_owned()
        } else {
            format!("{}_{}", self.namespace, table)
        };
        Identifier::new(identifier)
    }

    pub fn execute(&mut self, statement: &Statement) -> Result<()> {
        self.backend.execute(statement)
    }

    /// Execute explicit statements in order. SQL is never split on punctuation.
    pub fn execute_batch(&mut self, statements: &[Statement]) -> Result<()> {
        for statement in statements {
            self.execute(statement)?;
        }
        Ok(())
    }

    pub fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>> {
        self.backend.query(statement)
    }

    pub fn query_row(&mut self, statement: &Statement) -> Result<Vec<Value>> {
        let mut rows = self.query(statement)?;
        if rows.is_empty() {
            return Err(Error::NotFound);
        }
        Ok(rows.remove(0))
    }

    pub fn transaction<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.backend.begin(TransactionMode::Atomic)?;
        match op(self) {
            Ok(value) => {
                self.backend.commit()?;
                Ok(value)
            }
            Err(error) => {
                let _ = self.backend.rollback();
                Err(error)
            }
        }
    }
}

#[cfg(all(
    feature = "backend-redline",
    not(any(feature = "oracle-postgres", feature = "oracle-sqlite"))
))]
fn open_selected(dsn: &str) -> Result<Box<dyn Backend>> {
    adapter_redline::open(dsn)
}

#[cfg(all(
    feature = "oracle-postgres",
    not(any(feature = "backend-redline", feature = "oracle-sqlite"))
))]
fn open_selected(dsn: &str) -> Result<Box<dyn Backend>> {
    adapter_postgres::open(dsn)
}

#[cfg(all(
    feature = "oracle-sqlite",
    not(any(feature = "backend-redline", feature = "oracle-postgres"))
))]
fn open_selected(dsn: &str) -> Result<Box<dyn Backend>> {
    adapter_sqlite::open(dsn)
}
