use std::sync::Arc;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StorageClass {
    Null,
    Integer,
    Real,
    Text,
    Blob,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OwnedValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(Arc<str>),
    Blob(Arc<[u8]>),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a str),
    Blob(&'a [u8]),
}

impl OwnedValue {
    #[inline]
    pub fn storage_class(&self) -> StorageClass {
        match self {
            Self::Null => StorageClass::Null,
            Self::Integer(_) => StorageClass::Integer,
            Self::Real(_) => StorageClass::Real,
            Self::Text(_) => StorageClass::Text,
            Self::Blob(_) => StorageClass::Blob,
        }
    }

    #[inline]
    pub fn as_ref(&self) -> ValueRef<'_> {
        match self {
            Self::Null => ValueRef::Null,
            Self::Integer(v) => ValueRef::Integer(*v),
            Self::Real(v) => ValueRef::Real(*v),
            Self::Text(v) => ValueRef::Text(v),
            Self::Blob(v) => ValueRef::Blob(v),
        }
    }
}

impl<'a> ValueRef<'a> {
    #[inline]
    pub fn storage_class(self) -> StorageClass {
        match self {
            Self::Null => StorageClass::Null,
            Self::Integer(_) => StorageClass::Integer,
            Self::Real(_) => StorageClass::Real,
            Self::Text(_) => StorageClass::Text,
            Self::Blob(_) => StorageClass::Blob,
        }
    }

    pub fn to_owned(self) -> OwnedValue {
        match self {
            Self::Null => OwnedValue::Null,
            Self::Integer(v) => OwnedValue::Integer(v),
            Self::Real(v) => OwnedValue::Real(v),
            Self::Text(v) => OwnedValue::Text(Arc::from(v)),
            Self::Blob(v) => OwnedValue::Blob(Arc::from(v)),
        }
    }
}
