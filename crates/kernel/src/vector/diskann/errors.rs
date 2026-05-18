/// Build-time errors.
#[derive(Debug, PartialEq)]
pub enum BuildError {
    /// `vectors.len()` did not match `row_ids.len()`.
    Mismatch { vectors: usize, ids: usize },
    /// One of the input vectors had an unexpected dimension.
    DimMismatch { expected: usize, actual: usize },
    /// A coordinate was NaN or infinite. We refuse to build.
    NonFinite,
    /// The supplied [`super::DiskAnnParams`] were unusable; message gives the gist.
    InvalidParams(&'static str),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::Mismatch { vectors, ids } => {
                write!(f, "vector/row_id length mismatch: {vectors} vs {ids}")
            }
            BuildError::DimMismatch { expected, actual } => {
                write!(f, "dimension mismatch: expected {expected}, got {actual}")
            }
            BuildError::NonFinite => write!(f, "vector contained a non-finite coordinate"),
            BuildError::InvalidParams(reason) => write!(f, "invalid build params: {reason}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Search-time errors.
#[derive(Debug, PartialEq)]
pub enum SearchError {
    /// Query length did not match the index dim.
    DimMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::DimMismatch { expected, actual } => {
                write!(f, "query dim mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl std::error::Error for SearchError {}
