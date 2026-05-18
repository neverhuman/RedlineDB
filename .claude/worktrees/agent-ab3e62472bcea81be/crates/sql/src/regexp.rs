//! Lane SQL-D phase 10: REGEXP operator support.
//!
//! Provides a minimal `value REGEXP pattern` evaluator backed by the `regex`
//! crate. Compile errors are surfaced as `Error::UnsupportedSql` so callers
//! can distinguish them from `NULL` propagation. A small per-call cache could
//! be added later; for now we compile per evaluation, which keeps lifetimes
//! simple at the cost of a modest overhead. Patterns are typically literal
//! string constants, so this is acceptable for the SQLite-compat surface.

use std::collections::HashMap;
use std::sync::Mutex;

use regex::Regex;

use crate::error::{Error, Result};

static REGEX_CACHE: Mutex<Option<HashMap<String, Regex>>> = Mutex::new(None);

/// Compile `pattern` (with caching) and report whether `text` matches.
pub fn regex_match(text: &str, pattern: &str) -> Result<bool> {
    if let Some(re) = cached_regex(pattern) {
        return Ok(re.is_match(text));
    }
    let compiled = Regex::new(pattern)
        .map_err(|err| Error::UnsupportedSql(format!("invalid regex pattern: {err}")))?;
    let matched = compiled.is_match(text);
    insert_regex(pattern.to_owned(), compiled);
    Ok(matched)
}

fn cached_regex(pattern: &str) -> Option<Regex> {
    let guard = REGEX_CACHE.lock().ok()?;
    guard.as_ref().and_then(|map| map.get(pattern).cloned())
}

fn insert_regex(pattern: String, compiled: Regex) {
    if let Ok(mut guard) = REGEX_CACHE.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        if map.len() > 256 {
            // Bound the cache; clear and restart rather than implement an LRU
            // for what is intended to be a small set of distinct patterns.
            map.clear();
        }
        map.entry(pattern).or_insert(compiled);
    }
}
