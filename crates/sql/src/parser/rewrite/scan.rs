//! Shared SQL scanners for pre-parse rewrites.
#![allow(dead_code)]

pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle_lower: &[u8]) -> bool {
    find_ignore_ascii_case(haystack, needle_lower).is_some()
}

/// Allocation-free case-insensitive substring search that returns the
/// byte offset of the first match.
///
/// This is the indexed sibling of `contains_ignore_ascii_case`. It lets
/// parser rewrites locate ASCII SQL tokens without cloning the whole SQL
/// string into lowercase form first.
pub(crate) fn find_ignore_ascii_case(haystack: &str, needle_lower: &[u8]) -> Option<usize> {
    let hay = haystack.as_bytes();
    if needle_lower.is_empty() {
        return Some(0);
    }
    if hay.len() < needle_lower.len() {
        return None;
    }
    let head = needle_lower[0];
    let head_alt = match head {
        b'a'..=b'z' => head - 32,
        _ => head,
    };
    let end = hay.len() - needle_lower.len() + 1;
    let mut i = 0;
    while i < end {
        let b = hay[i];
        if b == head || b == head_alt {
            let mut matched = true;
            for j in 1..needle_lower.len() {
                let h = hay[i + j];
                let n = needle_lower[j];
                let eq = if n.is_ascii_lowercase() {
                    h == n || h == n - 32
                } else {
                    h == n
                };
                if !eq {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

pub(crate) fn strip_ignore_ascii_case_prefix<'a>(
    value: &'a str,
    prefix_lower: &[u8],
) -> Option<&'a str> {
    let bytes = value.as_bytes();
    if bytes.len() < prefix_lower.len() {
        return None;
    }
    for (byte, expected) in bytes.iter().zip(prefix_lower.iter()) {
        let matches = if expected.is_ascii_lowercase() {
            *byte == *expected || *byte == *expected - 32
        } else {
            *byte == *expected
        };
        if !matches {
            return None;
        }
    }
    Some(&value[prefix_lower.len()..])
}
