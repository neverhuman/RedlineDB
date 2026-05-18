//! SQLite JSON1 path parser and evaluator.
//!
//! Path grammar (matches SQLite's documented behaviour):
//!   path  := '$' segment*
//!   segment := '.' member | '[' index ']'
//!   member  := unquoted_ident | '"' quoted_chars '"'
//!   index   := digits | '#' | '#-' digits
//!
//! `$` alone refers to the document root. Bracket indices count from 0;
//! `[#]` refers to the element after the last (used for append in
//! `json_set`/`json_insert`); `[#-N]` counts from the end (so `[#-1]` is
//! the last element).

use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    /// Object key.
    Key(String),
    /// Zero-based array index.
    Index(usize),
    /// SQLite `#` token meaning "one past the last" (append position).
    Append,
    /// SQLite `#-N` token meaning N from the end (1 == last).
    FromEnd(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonPath {
    pub segments: Vec<PathSegment>,
}

impl JsonPath {
    /// Parse an SQLite JSON1 path string.
    pub fn parse(input: &str) -> Result<Self, PathError> {
        let bytes = input.as_bytes();
        if bytes.is_empty() {
            return Err(PathError::Empty);
        }
        if bytes[0] != b'$' {
            return Err(PathError::MissingRoot);
        }

        let mut segments = Vec::new();
        let mut i = 1usize;
        let len = bytes.len();
        while i < len {
            match bytes[i] {
                b'.' => {
                    i += 1;
                    let (seg, used) = parse_member(&input[i..])?;
                    segments.push(seg);
                    i += used;
                }
                b'[' => {
                    i += 1;
                    let (seg, used) = parse_index(&input[i..])?;
                    segments.push(seg);
                    i += used;
                    if i >= len || bytes[i] != b']' {
                        return Err(PathError::Unterminated);
                    }
                    i += 1;
                }
                _ => {
                    return Err(PathError::Syntax(format!(
                        "unexpected '{}'",
                        bytes[i] as char
                    )));
                }
            }
        }

        Ok(Self { segments })
    }

    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("$")?;
        for seg in &self.segments {
            match seg {
                PathSegment::Key(k) => {
                    if is_simple_ident(k) {
                        write!(f, ".{}", k)?;
                    } else {
                        write!(f, ".\"{}\"", k.replace('"', "\\\""))?;
                    }
                }
                PathSegment::Index(i) => write!(f, "[{}]", i)?,
                PathSegment::Append => f.write_str("[#]")?,
                PathSegment::FromEnd(n) => write!(f, "[#-{}]", n)?,
            }
        }
        Ok(())
    }
}

fn is_simple_ident(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_alphanumeric() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn parse_member(rest: &str) -> Result<(PathSegment, usize), PathError> {
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return Err(PathError::Syntax("expected member after '.'".into()));
    }
    if bytes[0] == b'"' {
        // Quoted member; parse until matching quote, supporting backslash
        // escapes for `\"` and `\\`.
        let mut out = String::new();
        let mut i = 1usize;
        while i < bytes.len() {
            match bytes[i] {
                b'"' => {
                    return Ok((PathSegment::Key(out), i + 1));
                }
                b'\\' if i + 1 < bytes.len() => {
                    out.push(bytes[i + 1] as char);
                    i += 2;
                }
                _ => {
                    let ch_len = utf8_char_len(&bytes[i..]);
                    out.push_str(&rest[i..i + ch_len]);
                    i += ch_len;
                }
            }
        }
        Err(PathError::Unterminated)
    } else {
        // Unquoted: alphanumerics, underscore, and any non-ASCII (so unicode
        // works without quoting).
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'.' || b == b'[' {
                break;
            }
            let ch_len = utf8_char_len(&bytes[i..]);
            i += ch_len;
        }
        if i == 0 {
            return Err(PathError::Syntax("empty member name".into()));
        }
        Ok((PathSegment::Key(rest[..i].to_string()), i))
    }
}

fn parse_index(rest: &str) -> Result<(PathSegment, usize), PathError> {
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return Err(PathError::Syntax("expected index after '['".into()));
    }
    if bytes[0] == b'#' {
        // Either `#` (append) or `#-N`.
        if bytes.len() >= 2 && bytes[1] == b'-' {
            let mut i = 2usize;
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                return Err(PathError::Syntax("expected digits after '#-'".into()));
            }
            let n: usize = rest[start..i]
                .parse()
                .map_err(|_| PathError::Syntax("bad index".into()))?;
            Ok((PathSegment::FromEnd(n), i))
        } else {
            Ok((PathSegment::Append, 1))
        }
    } else if bytes[0].is_ascii_digit() {
        let mut i = 0usize;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let n: usize = rest[..i]
            .parse()
            .map_err(|_| PathError::Syntax("bad index".into()))?;
        Ok((PathSegment::Index(n), i))
    } else {
        Err(PathError::Syntax(format!(
            "expected digits or '#' in index, found '{}'",
            bytes[0] as char
        )))
    }
}

fn utf8_char_len(bytes: &[u8]) -> usize {
    let b = bytes[0];
    if b < 0xC0 {
        // ASCII (b < 0x80) or stray continuation byte (0x80..=0xBF): always
        // advance by one byte to avoid panicking on malformed UTF-8.
        1
    } else if b < 0xE0 {
        2.min(bytes.len())
    } else if b < 0xF0 {
        3.min(bytes.len())
    } else {
        4.min(bytes.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    MissingRoot,
    Unterminated,
    Syntax(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => f.write_str("empty JSON path"),
            PathError::MissingRoot => f.write_str("JSON path must start with '$'"),
            PathError::Unterminated => f.write_str("unterminated JSON path token"),
            PathError::Syntax(msg) => write!(f, "JSON path syntax error: {msg}"),
        }
    }
}

/// Resolve a path against a JSON value, returning the referent (or None if
/// any segment is missing). For `Append` and `FromEnd(n)` past the end the
/// segment is treated as "missing" — those tokens are only meaningful for
/// `json_set`/`json_insert` mutation, not lookup.
pub fn resolve<'a>(root: &'a Value, path: &JsonPath) -> Option<&'a Value> {
    let mut current = root;
    for seg in &path.segments {
        match seg {
            PathSegment::Key(k) => match current {
                Value::Object(map) => match map.get(k) {
                    Some(v) => current = v,
                    None => return None,
                },
                _ => return None,
            },
            PathSegment::Index(i) => match current {
                Value::Array(arr) => match arr.get(*i) {
                    Some(v) => current = v,
                    None => return None,
                },
                _ => return None,
            },
            PathSegment::Append => return None,
            PathSegment::FromEnd(n) => match current {
                Value::Array(arr) => {
                    if *n == 0 || *n > arr.len() {
                        return None;
                    }
                    current = &arr[arr.len() - *n];
                }
                _ => return None,
            },
        }
    }
    Some(current)
}

/// Resolution mode for mutating operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationMode {
    /// `json_set`: overwrite if present, create if missing.
    Set,
    /// `json_insert`: write only if missing.
    Insert,
    /// `json_replace`: write only if present.
    Replace,
}

/// Apply a single (path, value) mutation to the root.
///
/// Returns `Ok(())` even if the operation is a no-op (e.g. `json_insert`
/// where the target already exists, or `json_replace` where it does not).
/// SQLite's behaviour is to silently skip in those cases.
pub fn mutate(
    root: &mut Value,
    path: &JsonPath,
    value: Value,
    mode: MutationMode,
) -> Result<(), PathError> {
    if path.is_root() {
        match mode {
            MutationMode::Set => *root = value,
            MutationMode::Insert => {
                // Root is always "present"; no-op.
            }
            MutationMode::Replace => *root = value,
        }
        return Ok(());
    }

    mutate_recursive(root, &path.segments, value, mode)
}

fn mutate_recursive(
    current: &mut Value,
    segments: &[PathSegment],
    value: Value,
    mode: MutationMode,
) -> Result<(), PathError> {
    let (seg, rest) = segments.split_first().expect("non-empty by caller");
    let is_leaf = rest.is_empty();

    match seg {
        PathSegment::Key(k) => {
            if !current.is_object() {
                // SQLite silently skips when the parent type doesn't match.
                return Ok(());
            }
            let map = current.as_object_mut().unwrap();
            if is_leaf {
                let exists = map.contains_key(k);
                match mode {
                    MutationMode::Set => {
                        map.insert(k.clone(), value);
                    }
                    MutationMode::Insert => {
                        if !exists {
                            map.insert(k.clone(), value);
                        }
                    }
                    MutationMode::Replace => {
                        if exists {
                            map.insert(k.clone(), value);
                        }
                    }
                }
            } else {
                match map.get_mut(k) {
                    Some(child) => mutate_recursive(child, rest, value, mode)?,
                    None => {
                        // Intermediate path missing: SQLite skips.
                    }
                }
            }
        }
        PathSegment::Index(i) => {
            if !current.is_array() {
                return Ok(());
            }
            let arr = current.as_array_mut().unwrap();
            if is_leaf {
                if *i < arr.len() {
                    match mode {
                        MutationMode::Set | MutationMode::Replace => arr[*i] = value,
                        MutationMode::Insert => {}
                    }
                } else {
                    // SQLite appends when the index is exactly arr.len() and
                    // the mode is Set/Insert; out-of-range otherwise is a
                    // no-op.
                    if *i == arr.len() && matches!(mode, MutationMode::Set | MutationMode::Insert) {
                        arr.push(value);
                    }
                }
            } else if let Some(child) = arr.get_mut(*i) {
                mutate_recursive(child, rest, value, mode)?;
            }
        }
        PathSegment::Append => {
            if !current.is_array() {
                return Ok(());
            }
            let arr = current.as_array_mut().unwrap();
            if is_leaf && matches!(mode, MutationMode::Set | MutationMode::Insert) {
                arr.push(value);
            }
            // For non-leaf the new element doesn't exist yet, so descent is
            // a no-op (SQLite behaviour).
        }
        PathSegment::FromEnd(n) => {
            if !current.is_array() {
                return Ok(());
            }
            let arr = current.as_array_mut().unwrap();
            if *n == 0 || *n > arr.len() {
                return Ok(());
            }
            let idx = arr.len() - *n;
            if is_leaf {
                match mode {
                    MutationMode::Set | MutationMode::Replace => arr[idx] = value,
                    MutationMode::Insert => {}
                }
            } else {
                let child = &mut arr[idx];
                mutate_recursive(child, rest, value, mode)?;
            }
        }
    }
    Ok(())
}

/// Remove the value at `path` from `root`. Returns true if something was
/// removed. `$` cannot be removed.
pub fn remove(root: &mut Value, path: &JsonPath) -> bool {
    if path.is_root() {
        return false;
    }
    remove_recursive(root, &path.segments)
}

fn remove_recursive(current: &mut Value, segments: &[PathSegment]) -> bool {
    let (seg, rest) = segments.split_first().expect("non-empty by caller");
    let is_leaf = rest.is_empty();

    match seg {
        PathSegment::Key(k) => {
            let Some(map) = current.as_object_mut() else {
                return false;
            };
            if is_leaf {
                map.remove(k).is_some()
            } else {
                match map.get_mut(k) {
                    Some(child) => remove_recursive(child, rest),
                    None => false,
                }
            }
        }
        PathSegment::Index(i) => {
            let Some(arr) = current.as_array_mut() else {
                return false;
            };
            if is_leaf {
                if *i < arr.len() {
                    arr.remove(*i);
                    true
                } else {
                    false
                }
            } else if let Some(child) = arr.get_mut(*i) {
                remove_recursive(child, rest)
            } else {
                false
            }
        }
        PathSegment::Append => false,
        PathSegment::FromEnd(n) => {
            let Some(arr) = current.as_array_mut() else {
                return false;
            };
            if *n == 0 || *n > arr.len() {
                return false;
            }
            let idx = arr.len() - *n;
            if is_leaf {
                arr.remove(idx);
                true
            } else {
                remove_recursive(&mut arr[idx], rest)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_root() {
        let p = JsonPath::parse("$").unwrap();
        assert!(p.is_root());
    }

    #[test]
    fn parses_dotted_member() {
        let p = JsonPath::parse("$.foo").unwrap();
        assert_eq!(p.segments, vec![PathSegment::Key("foo".into())]);
    }

    #[test]
    fn parses_indexed_array() {
        let p = JsonPath::parse("$[0]").unwrap();
        assert_eq!(p.segments, vec![PathSegment::Index(0)]);
    }

    #[test]
    fn parses_complex_path() {
        let p = JsonPath::parse("$.a[1].b").unwrap();
        assert_eq!(
            p.segments,
            vec![
                PathSegment::Key("a".into()),
                PathSegment::Index(1),
                PathSegment::Key("b".into())
            ]
        );
    }

    #[test]
    fn rejects_missing_root() {
        assert!(matches!(
            JsonPath::parse("foo"),
            Err(PathError::MissingRoot)
        ));
    }

    #[test]
    fn supports_unicode_keys() {
        let p = JsonPath::parse("$.café").unwrap();
        assert_eq!(p.segments, vec![PathSegment::Key("café".into())]);
    }

    #[test]
    fn supports_quoted_keys_with_dot() {
        let p = JsonPath::parse("$.\"a.b\"").unwrap();
        assert_eq!(p.segments, vec![PathSegment::Key("a.b".into())]);
    }

    #[test]
    fn parses_append_and_from_end() {
        let p = JsonPath::parse("$[#]").unwrap();
        assert_eq!(p.segments, vec![PathSegment::Append]);
        let p = JsonPath::parse("$[#-1]").unwrap();
        assert_eq!(p.segments, vec![PathSegment::FromEnd(1)]);
    }

    #[test]
    fn resolve_finds_nested() {
        let v = json!({"a": [1, 2, {"b": "c"}]});
        let p = JsonPath::parse("$.a[2].b").unwrap();
        assert_eq!(resolve(&v, &p), Some(&Value::String("c".into())));
    }

    #[test]
    fn mutate_set_creates_member() {
        let mut v = json!({"a": 1});
        let p = JsonPath::parse("$.b").unwrap();
        mutate(&mut v, &p, json!(2), MutationMode::Set).unwrap();
        assert_eq!(v, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn mutate_insert_skips_existing() {
        let mut v = json!({"a": 1});
        let p = JsonPath::parse("$.a").unwrap();
        mutate(&mut v, &p, json!(99), MutationMode::Insert).unwrap();
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn mutate_replace_skips_missing() {
        let mut v = json!({"a": 1});
        let p = JsonPath::parse("$.b").unwrap();
        mutate(&mut v, &p, json!(99), MutationMode::Replace).unwrap();
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn remove_drops_member() {
        let mut v = json!({"a": 1, "b": 2});
        let p = JsonPath::parse("$.a").unwrap();
        assert!(remove(&mut v, &p));
        assert_eq!(v, json!({"b": 2}));
    }
}
