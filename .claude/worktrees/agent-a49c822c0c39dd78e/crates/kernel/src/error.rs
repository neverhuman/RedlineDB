use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid checksum")]
    InvalidChecksum,

    #[error("invalid magic: expected {expected:#x}, got {actual:#x}")]
    InvalidMagic { expected: u32, actual: u32 },

    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u16),

    #[error("buffer too small: need {needed} bytes, got {actual}")]
    BufferTooSmall { needed: usize, actual: usize },

    #[error("corrupt page: {0}")]
    CorruptPage(&'static str),

    #[error("corrupt wal: {0}")]
    CorruptWal(&'static str),

    #[error("no free slot space on page")]
    PageFull,

    #[error("transaction is not visible in this snapshot")]
    NotVisible,

    #[error("write conflict")]
    WriteConflict,

    #[error("lock timeout")]
    LockTimeout,

    #[error("serialization failure")]
    SerializationFailure,

    #[error("unsupported isolation level")]
    UnsupportedIsolation,

    #[error("transaction is already closed")]
    TransactionClosed,

    #[error("catalog corruption: {0}")]
    CatalogCorrupt(&'static str),

    #[error("object already exists")]
    ObjectExists,

    #[error("object not found")]
    ObjectNotFound,

    #[error("column not found")]
    ColumnNotFound,

    #[error("schema changed")]
    SchemaChanged,

    #[error("constraint violation: {0}")]
    ConstraintViolation(&'static str),

    #[error("datatype mismatch")]
    DatatypeMismatch,

    #[error("unsupported ddl: {0}")]
    UnsupportedDdl(&'static str),

    #[error("invalid record: {0}")]
    InvalidRecord(&'static str),

    #[error("invalid jsonb: {0}")]
    InvalidJsonb(&'static str),

    #[error("invalid json path: {0}")]
    InvalidJsonPath(&'static str),

    #[error("vector error: {0}")]
    Vector(String),
}

impl From<crate::vector::VectorError> for Error {
    fn from(err: crate::vector::VectorError) -> Self {
        Error::Vector(err.to_string())
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Io(left), Self::Io(right)) => left.kind() == right.kind(),
            (Self::InvalidChecksum, Self::InvalidChecksum) => true,
            (
                Self::InvalidMagic {
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::InvalidMagic {
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => left_expected == right_expected && left_actual == right_actual,
            (Self::UnsupportedVersion(left), Self::UnsupportedVersion(right)) => left == right,
            (
                Self::BufferTooSmall {
                    needed: left_needed,
                    actual: left_actual,
                },
                Self::BufferTooSmall {
                    needed: right_needed,
                    actual: right_actual,
                },
            ) => left_needed == right_needed && left_actual == right_actual,
            (Self::CorruptPage(left), Self::CorruptPage(right)) => left == right,
            (Self::CorruptWal(left), Self::CorruptWal(right)) => left == right,
            (Self::PageFull, Self::PageFull) => true,
            (Self::NotVisible, Self::NotVisible) => true,
            (Self::WriteConflict, Self::WriteConflict) => true,
            (Self::LockTimeout, Self::LockTimeout) => true,
            (Self::SerializationFailure, Self::SerializationFailure) => true,
            (Self::UnsupportedIsolation, Self::UnsupportedIsolation) => true,
            (Self::TransactionClosed, Self::TransactionClosed) => true,
            (Self::CatalogCorrupt(left), Self::CatalogCorrupt(right)) => left == right,
            (Self::ObjectExists, Self::ObjectExists) => true,
            (Self::ObjectNotFound, Self::ObjectNotFound) => true,
            (Self::ColumnNotFound, Self::ColumnNotFound) => true,
            (Self::SchemaChanged, Self::SchemaChanged) => true,
            (Self::ConstraintViolation(left), Self::ConstraintViolation(right)) => left == right,
            (Self::DatatypeMismatch, Self::DatatypeMismatch) => true,
            (Self::UnsupportedDdl(left), Self::UnsupportedDdl(right)) => left == right,
            (Self::InvalidRecord(left), Self::InvalidRecord(right)) => left == right,
            (Self::InvalidJsonb(left), Self::InvalidJsonb(right)) => left == right,
            (Self::InvalidJsonPath(left), Self::InvalidJsonPath(right)) => left == right,
            (Self::Vector(left), Self::Vector(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for Error {}
