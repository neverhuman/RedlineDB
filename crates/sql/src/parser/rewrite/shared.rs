pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle_lower: &[u8]) -> bool {
    find_ignore_ascii_case(haystack, needle_lower).is_some()
}

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

pub(crate) fn find_top_level_keyword(
    upper: &str,
    bytes: &[u8],
    from: usize,
    kw: &str,
) -> Option<usize> {
    let mut i = from;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while i + kw.len() <= bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_str = Some(b);
                i += 1;
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && &upper[i..i + kw.len()] == kw {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub(crate) fn split_top_level_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => in_str = Some(b),
            b'(' => depth += 1,
            b')' => depth -= 1,
            b';' if depth == 0 => {
                out.push(sql[start..i].to_owned());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < bytes.len() {
        out.push(sql[start..].to_owned());
    }
    out
}

pub(crate) fn find_matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                if i + 1 < bytes.len() && bytes[i + 1] == q {
                    i += 2;
                    continue;
                }
                in_str = None;
            }
        } else {
            match b {
                b'\'' | b'"' => in_str = Some(b),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

pub(crate) fn find_matching_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    let mut in_str: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                if i + 1 < bytes.len() && bytes[i + 1] == q {
                    i += 2;
                    continue;
                }
                in_str = None;
            }
        } else {
            match b {
                b'\'' | b'"' => in_str = Some(b),
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

pub(crate) fn scan_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
    debug_assert_eq!(bytes[start], quote);
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

pub(crate) fn matches_keyword_ci(bytes: &[u8], pos: usize, keyword: &[u8]) -> bool {
    if pos + keyword.len() > bytes.len() {
        return false;
    }
    for (i, &k) in keyword.iter().enumerate() {
        if bytes[pos + i].to_ascii_uppercase() != k {
            return false;
        }
    }
    true
}

pub(crate) fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

pub(crate) fn trim_trailing_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let bytes = text.as_bytes();
    if bytes.len() < keyword.len() {
        return None;
    }
    let key_start = bytes.len() - keyword.len();
    for (i, k) in keyword.bytes().enumerate() {
        if bytes[key_start + i].to_ascii_uppercase() != k.to_ascii_uppercase() {
            return None;
        }
    }
    if key_start == 0 || !bytes[key_start - 1].is_ascii_whitespace() {
        return None;
    }
    let mut end = key_start;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    Some(&text[..end])
}

pub(crate) fn keyword_at_boundary(_lower: &str, bytes: &[u8], at: usize, keyword: &str) -> bool {
    let end = at + keyword.len();
    end <= bytes.len()
        && bytes[at..end].eq_ignore_ascii_case(keyword.as_bytes())
        && (at == 0 || !is_word_char(bytes[at - 1]))
        && (end >= bytes.len() || !is_word_char(bytes[end]))
}

pub(crate) fn sql_gap_is_trivia(gap: &str) -> bool {
    skip_sql_trivia(gap.as_bytes(), 0) == Some(gap.len())
}

pub(crate) fn skip_sql_trivia(bytes: &[u8], mut i: usize) -> Option<usize> {
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"--" {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"/*" {
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                i += 1;
            }
            if i + 1 >= bytes.len() {
                return None;
            }
            i += 2;
            continue;
        }
        return Some(i);
    }
}

pub(crate) fn skip_ws_bytes(bytes: &[u8], mut from: usize) -> usize {
    while from < bytes.len() && bytes[from].is_ascii_whitespace() {
        from += 1;
    }
    from
}
