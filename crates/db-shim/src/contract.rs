/// Backend-neutral values supported by the governed operation corpus.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// Stable error categories. Backend-specific error types never cross the application boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbError {
    Config(String),
    Connection(String),
    Protocol(String),
    Query(String),
    Unsupported(String),
    Contract(String),
    NotFound,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Config(message) => write!(f, "config: {message}"),
            DbError::Connection(message) => write!(f, "connection: {message}"),
            DbError::Protocol(message) => write!(f, "protocol: {message}"),
            DbError::Query(message) => write!(f, "query: {message}"),
            DbError::Unsupported(message) => write!(f, "unsupported: {message}"),
            DbError::Contract(message) => write!(f, "contract: {message}"),
            DbError::NotFound => write!(f, "query returned no rows"),
        }
    }
}

impl std::error::Error for DbError {}

pub type Error = DbError;
pub type Result<T> = std::result::Result<T, Error>;

/// Capabilities required by the governed operation corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub atomic_transactions: bool,
    pub binary_values: bool,
    pub parameterized_statements: bool,
}

pub const GOVERNED_CAPABILITIES: Capabilities = Capabilities {
    atomic_transactions: true,
    binary_values: true,
    parameterized_statements: true,
};

/// The only transaction semantic promised by the governed contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionMode {
    Atomic,
}

/// Validated SQL identifier. Its bytes cannot be constructed without the identifier allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier(String);

impl Identifier {
    pub(crate) fn new(identifier: String) -> Result<Self> {
        validate_identifier(&identifier, false)?;
        Ok(Self(identifier))
    }
}

/// Structurally distinct pieces of a governed statement.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlPart {
    /// Static SQL syntax from reviewed source.
    Syntax(&'static str),
    /// An allowlisted identifier.
    Identifier(Identifier),
    /// A typed parameter rendered with the selected adapter's placeholder syntax.
    Parameter(Value),
}

/// A statement composed from static syntax, validated identifiers, and typed values.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    fragments: Vec<String>,
    params: Vec<Value>,
}

impl Statement {
    pub fn plain(sql: &'static str) -> Self {
        Self {
            fragments: vec![sql.to_owned()],
            params: Vec::new(),
        }
    }

    pub fn compose(parts: impl IntoIterator<Item = SqlPart>) -> Result<Self> {
        let mut fragments = vec![String::new()];
        let mut params = Vec::new();
        let mut part_count = 0_usize;
        for part in parts {
            part_count += 1;
            let fragment = fragments
                .last_mut()
                .ok_or_else(|| Error::Contract("statement fragment state is empty".to_owned()))?;
            match part {
                SqlPart::Syntax(syntax) => fragment.push_str(syntax),
                SqlPart::Identifier(identifier) => fragment.push_str(&identifier.0),
                SqlPart::Parameter(value) => {
                    params.push(value);
                    fragments.push(String::new());
                }
            }
        }
        if part_count == 0 {
            return Err(Error::Config(
                "statement must contain at least one part".to_owned(),
            ));
        }
        Ok(Self { fragments, params })
    }

    pub fn fragments(&self) -> &[String] {
        &self.fragments
    }

    pub fn params(&self) -> &[Value] {
        &self.params
    }

    pub(crate) fn render(&self, style: PlaceholderStyle) -> String {
        let mut sql = String::new();
        for (index, fragment) in self.fragments.iter().enumerate() {
            sql.push_str(fragment);
            if index < self.params.len() {
                match style {
                    #[cfg(any(feature = "backend-redline", feature = "oracle-sqlite", test))]
                    PlaceholderStyle::Question => sql.push('?'),
                    #[cfg(any(feature = "oracle-postgres", test))]
                    PlaceholderStyle::DollarNumbered => {
                        sql.push('$');
                        sql.push_str(&(index + 1).to_string());
                    }
                }
            }
        }
        sql
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PlaceholderStyle {
    #[cfg(any(feature = "backend-redline", feature = "oracle-sqlite", test))]
    Question,
    #[cfg(any(feature = "oracle-postgres", test))]
    DollarNumbered,
}

/// Provider contract implemented independently by every adapter.
pub trait Backend {
    fn capabilities(&self) -> Capabilities;
    fn execute(&mut self, statement: &Statement) -> Result<u64>;
    fn query(&mut self, statement: &Statement) -> Result<Vec<Vec<Value>>>;
    fn begin(&mut self, mode: TransactionMode) -> Result<()>;
    fn commit(&mut self) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
}

pub(crate) fn validate_identifier(identifier: &str, allow_empty: bool) -> Result<()> {
    if allow_empty && identifier.is_empty() {
        return Ok(());
    }
    let mut chars = identifier.chars();
    if !matches!(chars.next(), Some('a'..='z'))
        || !chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(Error::Config(format!(
            "identifier {identifier:?} must match [a-z][a-z0-9_]*"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_statement_is_rejected() {
        let error = Statement::compose(Vec::<SqlPart>::new()).unwrap_err();
        assert!(matches!(error, Error::Config(message) if message.contains("at least one part")));
    }

    #[test]
    fn identifier_validation_rejects_sql_syntax() {
        for invalid in ["Orders", "orders-items", "orders;drop", "1orders"] {
            let error = validate_identifier(invalid, false).unwrap_err();
            assert!(matches!(error, Error::Config(_)));
        }
    }

    #[test]
    fn renderer_places_only_structural_parameters() {
        let statement = Statement::compose([
            SqlPart::Syntax("SELECT * FROM items WHERE id = "),
            SqlPart::Parameter(Value::Integer(7)),
            SqlPart::Syntax(" AND name = "),
            SqlPart::Parameter(Value::Text("? $1".to_owned())),
        ])
        .unwrap();
        assert_eq!(
            statement.render(PlaceholderStyle::Question),
            "SELECT * FROM items WHERE id = ? AND name = ?"
        );
        assert_eq!(
            statement.render(PlaceholderStyle::DollarNumbered),
            "SELECT * FROM items WHERE id = $1 AND name = $2"
        );
    }
}
