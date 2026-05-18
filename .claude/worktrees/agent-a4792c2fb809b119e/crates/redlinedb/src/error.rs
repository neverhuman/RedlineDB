use std::borrow::Cow;
use std::fmt::{Display, Formatter};

use redlinedb_kernel::error as kernel_error;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    Ok = 0,
    Error = 1,
    Internal = 2,
    Permission = 3,
    Abort = 4,
    Busy = 5,
    Locked = 6,
    NoMem = 7,
    ReadOnly = 8,
    Interrupt = 9,
    IoErr = 10,
    Corrupt = 11,
    NotFound = 12,
    Full = 13,
    CantOpen = 14,
    Protocol = 15,
    Empty = 16,
    Schema = 17,
    TooBig = 18,
    Constraint = 19,
    Mismatch = 20,
    Misuse = 21,
    Auth = 23,
    Range = 25,
    NotDatabase = 26,
    Row = 100,
    Done = 101,
    Unsupported = 1002,
}

#[derive(Debug)]
pub struct Error {
    code: ErrorCode,
    message: Cow<'static, str>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        code: ErrorCode,
        message: impl Into<Cow<'static, str>>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        self.message.as_ref()
    }

    pub fn unsupported(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorCode::Unsupported, message)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code as i32, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

impl From<redlinedb_sql::Error> for Error {
    fn from(value: redlinedb_sql::Error) -> Self {
        use redlinedb_sql::Error as SqlError;
        match value {
            SqlError::Kernel(kernel_error::Error::LockTimeout) => {
                Self::new(ErrorCode::Busy, "lock timeout")
            }
            SqlError::Kernel(kernel_error::Error::SerializationFailure) => {
                Self::new(ErrorCode::Busy, "serialization failure")
            }
            SqlError::Kernel(kernel_error::Error::WriteConflict) => {
                Self::new(ErrorCode::Locked, "write conflict")
            }
            SqlError::Kernel(kernel_error::Error::ObjectNotFound)
            | SqlError::UnknownTable(_)
            | SqlError::UnknownColumn(_) => Self::new(ErrorCode::NotFound, value.to_string()),
            SqlError::Kernel(kernel_error::Error::ObjectExists) => {
                Self::new(ErrorCode::Constraint, value.to_string())
            }
            SqlError::Kernel(kernel_error::Error::DatatypeMismatch)
            | SqlError::DatatypeMismatch => Self::new(ErrorCode::Mismatch, value.to_string()),
            SqlError::Kernel(kernel_error::Error::ConstraintViolation(_))
            | SqlError::ConstraintViolation(_) => {
                Self::new(ErrorCode::Constraint, value.to_string())
            }
            SqlError::CommitMaybeCommitted => Self::new(ErrorCode::IoErr, value.to_string()),
            SqlError::TransactionState(_) | SqlError::ParameterOutOfRange(_) => {
                Self::new(ErrorCode::Misuse, value.to_string())
            }
            SqlError::Parse(_) => Self::new(ErrorCode::Error, value.to_string()),
            SqlError::UnsupportedSql(_) => Self::new(ErrorCode::Unsupported, value.to_string()),
            other => Self::with_source(ErrorCode::Error, other.to_string(), other),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        let code = match value.kind() {
            std::io::ErrorKind::NotFound => ErrorCode::NotFound,
            std::io::ErrorKind::PermissionDenied => ErrorCode::Permission,
            std::io::ErrorKind::AlreadyExists => ErrorCode::Busy,
            std::io::ErrorKind::WouldBlock => ErrorCode::Busy,
            _ => ErrorCode::IoErr,
        };
        Self::with_source(code, value.to_string(), value)
    }
}
