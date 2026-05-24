//! Thread-local string interning for hot SQL identifiers and short text
//! literals.
//!
//! The expression evaluator repeatedly constructs `Arc<str>` payloads for
//! identifiers, type-name strings, and small text literals projected during
//! query execution (see `expr::scalar::row::lookup::lookup_table_column`
//! at lookup time and `expr::mod` at literal eval). Each construction does
//! a heap allocation plus a `memcpy`. For the same identifier seen N times
//! within one statement, the same allocation is repeated N times.
//!
//! `intern_arc` maintains a bounded thread-local FIFO cache that collapses
//! repeat allocations into pointer copies. The cache is scoped per worker
//! thread and bounded by `INTERN_CAP` entries; strings longer than
//! `INTERN_MAX_LEN` bypass the cache to avoid pinning large blobs.
//!
//! Lifetime: the cache lives for the duration of the worker thread. There
//! is no per-statement clear (Arc reference-counting handles eviction
//! naturally), but a `clear` is exposed for tests and explicit teardown.

use std::cell::RefCell;
use std::sync::Arc;

/// Maximum number of distinct interned strings retained per worker thread.
/// Sized to comfortably hold the column-name + type-name + small-literal
/// working set of a complex query without paying eviction churn.
const INTERN_CAP: usize = 256;

/// Strings longer than this many bytes bypass the cache. Limits worst-case
/// memory pinning if a user supplies a large text literal.
const INTERN_MAX_LEN: usize = 64;

thread_local! {
    static INTERN_CACHE: RefCell<Vec<(Box<str>, Arc<str>)>> =
        RefCell::new(Vec::with_capacity(INTERN_CAP));
}

/// Return an `Arc<str>` for `s`. Equal strings within a worker thread are
/// guaranteed to return the same `Arc` (pointer-equal). Strings longer than
/// `INTERN_MAX_LEN` are returned as fresh `Arc::from` allocations.
#[inline]
pub fn intern_arc(s: &str) -> Arc<str> {
    if s.len() > INTERN_MAX_LEN {
        return Arc::from(s);
    }
    INTERN_CACHE.with(|cache| {
        let mut items = cache.borrow_mut();
        if let Some((_, arc)) = items.iter().find(|(k, _)| k.as_ref() == s) {
            return Arc::clone(arc);
        }
        if items.len() >= INTERN_CAP {
            items.remove(0);
        }
        let arc: Arc<str> = Arc::from(s);
        items.push((Box::from(s), Arc::clone(&arc)));
        arc
    })
}

/// Drop all entries from the thread-local intern cache. Test-only.
#[cfg(test)]
pub(crate) fn clear() {
    INTERN_CACHE.with(|cache| cache.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interns_short_strings() {
        clear();
        let a = intern_arc("col_name");
        let b = intern_arc("col_name");
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn bypasses_long_strings() {
        clear();
        let long = "x".repeat(INTERN_MAX_LEN + 1);
        let a = intern_arc(&long);
        let b = intern_arc(&long);
        assert!(!Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn evicts_when_full() {
        clear();
        let first = intern_arc("first");
        for i in 0..INTERN_CAP {
            let s = format!("filler_{i:03}");
            let _ = intern_arc(&s);
        }
        let first_again = intern_arc("first");
        assert!(!Arc::ptr_eq(&first, &first_again));
    }
}
