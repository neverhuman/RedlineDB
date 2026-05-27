//! Per-column storage for a `Morsel`. Variants stay narrow: the planner
//! picks the right shape at bind time so the hot path is a single match
//! per kernel invocation.

use redlinedb_kernel::catalog::ValueRef;
use smallvec::SmallVec;

use super::arena::BytesArena;

#[derive(Debug)]
pub enum ColumnBatch<'a> {
    I64(SmallVec<[i64; 256]>),
    F64(SmallVec<[f64; 256]>),
    Text(BytesArena<'a>),
    Blob(BytesArena<'a>),
    /// All-NULL column. Carries no payload; consumers must consult the
    /// morsel-level validity bitmap for true row count.
    Null {
        len: usize,
    },
}

impl<'a> ColumnBatch<'a> {
    pub fn len(&self) -> usize {
        match self {
            Self::I64(v) => v.len(),
            Self::F64(v) => v.len(),
            Self::Text(a) => a.len(),
            Self::Blob(a) => a.len(),
            Self::Null { len } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> Option<ValueRef<'_>> {
        match self {
            Self::I64(v) => v.get(i).copied().map(ValueRef::Integer),
            Self::F64(v) => v.get(i).copied().map(ValueRef::Real),
            Self::Text(a) => a
                .get(i)
                .and_then(|b| std::str::from_utf8(b).ok())
                .map(ValueRef::Text),
            Self::Blob(a) => a.get(i).map(ValueRef::Blob),
            Self::Null { len } => {
                if i < *len {
                    Some(ValueRef::Null)
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;
    use smallvec::smallvec;

    #[test]
    fn i64_column_len_and_get() {
        let c: ColumnBatch<'_> = ColumnBatch::I64(smallvec![1i64, 2, 3]);
        assert_eq!(c.len(), 3);
        assert!(matches!(c.get(0), Some(ValueRef::Integer(1))));
        assert!(matches!(c.get(2), Some(ValueRef::Integer(3))));
        assert!(c.get(3).is_none());
    }

    #[test]
    fn text_column_len_and_get() {
        let bump = Bump::new();
        let mut arena = BytesArena::new(&bump, 2);
        arena.push(b"alpha");
        arena.push(b"beta");
        let c = ColumnBatch::Text(arena);
        assert_eq!(c.len(), 2);
        assert!(matches!(c.get(0), Some(ValueRef::Text("alpha"))));
        assert!(matches!(c.get(1), Some(ValueRef::Text("beta"))));
    }

    #[test]
    fn null_column_yields_null() {
        let c: ColumnBatch<'_> = ColumnBatch::Null { len: 5 };
        assert_eq!(c.len(), 5);
        assert!(matches!(c.get(0), Some(ValueRef::Null)));
        assert!(c.get(5).is_none());
    }
}
