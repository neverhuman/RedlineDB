use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DbName {
    original: Arc<str>,
    folded: Arc<str>,
}

impl DbName {
    pub fn new(input: impl AsRef<str>) -> Self {
        let input = input.as_ref();
        let mut folded = String::with_capacity(input.len());
        for byte in input.bytes() {
            let mapped = if byte.is_ascii_uppercase() {
                byte.to_ascii_lowercase()
            } else {
                byte
            };
            folded.push(mapped as char);
        }
        Self {
            original: Arc::from(input),
            folded: Arc::from(folded),
        }
    }

    #[inline]
    pub fn original(&self) -> &str {
        &self.original
    }

    #[inline]
    pub fn folded(&self) -> &str {
        &self.folded
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct QualifiedName {
    pub schema: DbName,
    pub name: DbName,
}
