use redlinedb_kernel::Error as KernelError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("unsupported sql: {0}")]
    UnsupportedSql(String),

    #[error("bind error: {0}")]
    Bind(String),

    #[error("unknown table: {0}")]
    UnknownTable(String),

    #[error("unknown column: {0}")]
    UnknownColumn(String),

    #[error("ambiguous column: {0}")]
    AmbiguousColumn(String),

    #[error("parameter out of range: {0}")]
    ParameterOutOfRange(usize),

    #[error("schema changed")]
    SchemaChanged,

    #[error("transaction state error: {0}")]
    TransactionState(&'static str),

    #[error("commit outcome uncertain")]
    CommitMaybeCommitted,

    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("datatype mismatch")]
    DatatypeMismatch,
}

pub type Result<T> = std::result::Result<T, Error>;

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Kernel(a), Self::Kernel(b)) => a == b,
            (Self::Parse(a), Self::Parse(b)) => a == b,
            (Self::UnsupportedSql(a), Self::UnsupportedSql(b)) => a == b,
            (Self::Bind(a), Self::Bind(b)) => a == b,
            (Self::UnknownTable(a), Self::UnknownTable(b)) => a == b,
            (Self::UnknownColumn(a), Self::UnknownColumn(b)) => a == b,
            (Self::AmbiguousColumn(a), Self::AmbiguousColumn(b)) => a == b,
            (Self::ParameterOutOfRange(a), Self::ParameterOutOfRange(b)) => a == b,
            (Self::SchemaChanged, Self::SchemaChanged) => true,
            (Self::TransactionState(a), Self::TransactionState(b)) => a == b,
            (Self::CommitMaybeCommitted, Self::CommitMaybeCommitted) => true,
            (Self::ConstraintViolation(a), Self::ConstraintViolation(b)) => a == b,
            (Self::DatatypeMismatch, Self::DatatypeMismatch) => true,
            _ => false,
        }
    }
}

impl Eq for Error {}

impl From<sqlparser::parser::ParserError> for Error {
    fn from(value: sqlparser::parser::ParserError) -> Self {
        Self::Parse(value.to_string())
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::UnsupportedSql(value.to_owned())
    }
}
