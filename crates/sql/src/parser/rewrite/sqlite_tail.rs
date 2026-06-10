use super::shared::*;

pub(crate) fn rewrite_glob_to_function(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len() + 32);
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        // Skip string literals verbatim so we never rewrite "GLOB" inside
        // user data.
        if c == b'\'' {
            let end = scan_quoted(bytes, i, b'\'');
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'"' {
            let end = scan_quoted(bytes, i, b'"');
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
            // Line comment — copy verbatim to newline.
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|n| i + n)
                .unwrap_or(bytes.len());
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            // Block comment.
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            let end = (j + 2).min(bytes.len());
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        // Match `GLOB` only when it is acting as the BINARY OPERATOR — i.e.
        // surrounded by whitespace and *not* immediately followed by `(`
        // (which would make it the `glob(pattern, value)` function call).
        // Function-call form is parsed natively, so don't rewrite it.
        if matches_keyword_ci(bytes, i, b"GLOB")
            && (i == 0 || !is_word_char(bytes[i - 1]))
            && (i + 4 == bytes.len() || !is_word_char(bytes[i + 4]))
            && bytes.get(i + 4) != Some(&b'(')
            && (i + 4 < bytes.len() && bytes[i + 4].is_ascii_whitespace())
            && (i > 0 && bytes[i - 1].is_ascii_whitespace())
        {
            // Strip any trailing whitespace from `out` so we can pattern-
            // match against the immediately-preceding tokens.
            while let Some(last) = out.chars().last() {
                if last.is_whitespace() {
                    out.pop();
                } else {
                    break;
                }
            }
            // Detect a trailing `NOT` so we can wrap the rewrite in NOT.
            let negate = trim_trailing_keyword_ci(&out, "NOT").is_some();
            if negate {
                if let Some(prefix) = trim_trailing_keyword_ci(&out, "NOT") {
                    out.truncate(prefix.len());
                }
            }
            // Strip residual whitespace before the LHS atom.
            while let Some(last) = out.chars().last() {
                if last.is_whitespace() {
                    out.pop();
                } else {
                    break;
                }
            }
            // Locate the LHS atom in what we've buffered so far.
            let out_bytes = out.as_bytes();
            let lhs_end = out_bytes.len();
            let lhs_atom_start = match find_atom_start(out_bytes, lhs_end) {
                Some(s) => s,
                None => {
                    if negate {
                        out.push_str("NOT");
                    }
                    out.push(' ');
                    out.push(c as char);
                    i += 1;
                    continue;
                }
            };
            let lhs_atom = out[lhs_atom_start..lhs_end].to_owned();
            // Now find the RHS atom in the input.
            let mut j = i + 4;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let rhs_end = match find_atom_end(bytes, j) {
                Some(e) => e,
                None => {
                    if negate {
                        out.push_str("NOT");
                    }
                    out.push(' ');
                    out.push(c as char);
                    i += 1;
                    continue;
                }
            };
            let rhs_atom = std::str::from_utf8(&bytes[j..rhs_end]).unwrap_or("");
            // Trim the LHS atom out of `out`, then rebuild as
            // [prefix] [NOT ]glob(<rhs>, <lhs>)
            out.truncate(lhs_atom_start);
            if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                out.push(' ');
            }
            if negate {
                out.push_str("NOT ");
            }
            out.push_str("glob(");
            out.push_str(rhs_atom);
            out.push(',');
            out.push_str(&lhs_atom);
            out.push(')');
            i = rhs_end;
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    out
}

pub(crate) fn scan_quoted(bytes: &[u8], start: usize, quote: u8) -> usize {
    debug_assert_eq!(bytes[start], quote);
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == quote {
            // SQLite uses doubled-quote escaping.
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

/// Working backwards from `end`, find the start of an "atom" suitable as a
/// GLOB operand: a quoted string, NULL literal, parenthesized group,
/// numeric literal, or simple identifier. Returns `None` if the preceding
/// text doesn't look like a clean atom (e.g. mid-expression).
pub(crate) fn find_atom_start(bytes: &[u8], end: usize) -> Option<usize> {
    let mut i = end;
    // Skip trailing whitespace.
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 {
        return None;
    }
    let last = bytes[i - 1];
    // Quoted string atom: scan forward from each candidate opener to
    // confirm it ends exactly at `i`. Doubled-quote escapes (`''` inside
    // a `'`-quoted string) are handled by `scan_quoted`.
    if last == b'\'' || last == b'"' {
        let quote = last;
        let mut candidate = i - 1;
        // Walk back to the earliest possible opener and forward-scan to
        // verify. The earliest opener is the first `quote` byte at the
        // start of a run.
        loop {
            if candidate == 0 {
                if bytes[0] == quote && scan_quoted(bytes, 0, quote) == i {
                    return Some(0);
                }
                return None;
            }
            candidate -= 1;
            if bytes[candidate] == quote {
                // Could be either an opener or part of `''` escape.
                let prev = if candidate > 0 {
                    bytes[candidate - 1]
                } else {
                    0
                };
                if prev == quote {
                    // We're inside a doubled-quote pair; skip both.
                    if candidate == 0 {
                        return None;
                    }
                    candidate -= 1;
                    continue;
                }
                // Candidate is at an opener if scan_quoted from here
                // lands exactly on `i`.
                if scan_quoted(bytes, candidate, quote) == i {
                    // Also allow leading `x'...'` blob literal: if the
                    // byte before is `x` or `X`, include it in the atom.
                    if quote == b'\''
                        && candidate > 0
                        && (bytes[candidate - 1] == b'x' || bytes[candidate - 1] == b'X')
                    {
                        return Some(candidate - 1);
                    }
                    return Some(candidate);
                }
            }
        }
    }
    // Parenthesized group: scan back balancing.
    if last == b')' {
        let mut depth = 1i32;
        let mut j = i - 1;
        while j > 0 {
            j -= 1;
            match bytes[j] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    // Identifier / NULL / numeric: scan back while alphanumeric / `_` /
    // `.` (for qualified names). Allow leading `x'...'` blob literal.
    let mut j = i;
    while j > 0 {
        let b = bytes[j - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            j -= 1;
        } else {
            break;
        }
    }
    if j < i { Some(j) } else { None }
}

/// Forward equivalent of `find_atom_start`: pick out the end of an atom
/// starting at `start`. Returns `None` if no recognisable atom is present.
pub(crate) fn find_atom_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start >= bytes.len() {
        return None;
    }
    let first = bytes[start];
    if first == b'\'' || first == b'"' {
        return Some(scan_quoted(bytes, start, first));
    }
    // `x'01ab'` style blob literal.
    if (first == b'x' || first == b'X') && bytes.get(start + 1) == Some(&b'\'') {
        return Some(scan_quoted(bytes, start + 1, b'\''));
    }
    if first == b'(' {
        // Balance to matching ).
        let mut depth = 1i32;
        let mut j = start + 1;
        while j < bytes.len() {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j + 1);
                    }
                }
                _ => {}
            }
            j += 1;
        }
        return None;
    }
    // Identifier / numeric / NULL: consume alphanumeric / `_` / `.`.
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' {
            j += 1;
        } else {
            break;
        }
    }
    if j > start { Some(j) } else { None }
}

/// If `text` ends with the given uppercase keyword on a word boundary
/// preceded by whitespace, return the prefix excluding the keyword and
/// its leading whitespace.
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
    // Must be preceded by whitespace (or be at the very start, though
    // that'd be a degenerate GLOB).
    if key_start == 0 || !bytes[key_start - 1].is_ascii_whitespace() {
        return None;
    }
    // Trim back the whitespace too.
    let mut end = key_start;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    Some(&text[..end])
}

/// has a single index implementation, so we strip the access-method
/// hint pre-parse. JSONB opclass markers inside the column list
/// (`jsonb_path_ops`, `jsonb_ops`) are also dropped — they only affect
/// physical layout, not query semantics.
pub(crate) fn strip_create_index_using_clause(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let lower: Vec<u8> = bytes.iter().map(|b| b.to_ascii_lowercase()).collect();
    if !lower.windows(b"create".len()).any(|w| w == b"create") {
        return sql.to_owned();
    }
    let mut out = String::with_capacity(sql.len());
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            out.push(b as char);
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    out.push(quote as char);
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                out.push(b as char);
                i += 1;
            }
            _ => {
                // Match `USING <ident>` outside strings — strip both tokens.
                if (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                    && matches_word_ci(&lower, i, b"using")
                {
                    let mut j = i + 5;
                    // Skip whitespace.
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    // Capture the method-name identifier.
                    let name_start = j;
                    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
                    {
                        j += 1;
                    }
                    if j > name_start {
                        // Drop "USING <name>" entirely; eat the trailing
                        // whitespace too so we don't leave a double space.
                        while j < bytes.len() && bytes[j] == b' ' {
                            j += 1;
                        }
                        if !out.ends_with(' ') {
                            out.push(' ');
                        }
                        i = j;
                        continue;
                    }
                }
                // Match `<col> jsonb_path_ops` / `jsonb_ops` opclass marker.
                let mut stripped_marker = false;
                for marker in ["jsonb_path_ops", "jsonb_ops"] {
                    let mlen = marker.len();
                    if (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
                        && matches_word_ci(&lower, i, marker.as_bytes())
                    {
                        // Trailing must be punctuation/whitespace/end.
                        let after = i + mlen;
                        let ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
                        if ok {
                            i += mlen;
                            // Eat a single leading space we may have just emitted.
                            if out.ends_with(' ') {
                                out.pop();
                            }
                            stripped_marker = true;
                            break;
                        }
                    }
                }
                if stripped_marker {
                    continue;
                }
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

pub(crate) fn matches_word_ci(lower: &[u8], start: usize, needle: &[u8]) -> bool {
    if start + needle.len() > lower.len() {
        return false;
    }
    if &lower[start..start + needle.len()] != needle {
        return false;
    }
    let after = start + needle.len();
    after >= lower.len() || !lower[after].is_ascii_alphanumeric()
}

/// Returns `true` when `sql` contains a `?`, `?|`, or `?&` token that is
/// outside string/comment context and not directly followed by digits
/// (a placeholder). Used to gate the expensive JSONB-operator rewriter.
pub(crate) fn has_jsonb_question_op(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'?' => {
                // Skip `?<digit>` placeholders (`?1`, `?2`, ...).
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    i += 1;
                    continue;
                }
                if jsonb_question_op_shape(bytes, i) {
                    return true;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    false
}

/// True if the `?` byte at position `i` looks like a JSONB containment
/// operator (`?`, `?|`, `?&`) rather than a SQL positional placeholder.
///
/// Disambiguation rule: only treat `?` as a JSONB operator when the
/// right-hand side is one of the documented JSONB RHS shapes — a string
/// literal for `?`, or `ARRAY[...]` for `?|` / `?&`. Anything else
/// (including bare `?` followed by a SQL keyword, closing paren, comma,
/// or end-of-input) is the parameter placeholder and must be left alone.
pub(crate) fn jsonb_question_op_shape(bytes: &[u8], i: usize) -> bool {
    let next = bytes.get(i + 1).copied();
    let after_op = match next {
        Some(b'|') | Some(b'&') => i + 2,
        _ => i + 1,
    };
    let mut j = after_op;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= bytes.len() {
        return false;
    }
    match next {
        Some(b'|') | Some(b'&') => {
            // `?|` / `?&` require `ARRAY[`.
            let prefix = b"ARRAY[";
            if j + prefix.len() > bytes.len() {
                return false;
            }
            bytes[j..j + prefix.len()]
                .iter()
                .zip(prefix.iter())
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        }
        _ => bytes[j] == b'\'',
    }
}

/// Rewrite JSONB question-mark operators (`?`, `?|`, `?&`) into the
/// equivalent function calls `jsonb_exists`, `jsonb_exists_any`, and
/// `jsonb_exists_all`. SQLiteDialect tokenises `?` as a positional
/// placeholder, so we rewrite the surface SQL before the parser runs.
///
/// Recognised shapes (left operand is the longest balanced expression
/// preceding the `?`, right operand is the literal or `ARRAY[...]`
/// expression that follows):
///   `JSON ? 'key'`              → `jsonb_exists(JSON, 'key')`
///   `JSON ?| ARRAY['a','b']`    → `jsonb_exists_any(JSON, 'a', 'b')`
///   `JSON ?& ARRAY['a','b']`    → `jsonb_exists_all(JSON, 'a', 'b')`
pub(crate) fn rewrite_jsonb_question_ops(sql: &str) -> String {
    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len() + 32);
    let mut i = 0usize;
    let mut in_string: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            out.push(b as char);
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    out.push(quote as char);
                    i += 2;
                    continue;
                }
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                out.push(b as char);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                while i < bytes.len() {
                    out.push(bytes[i] as char);
                    if bytes[i] == b'/' && i > 0 && bytes[i - 1] == b'*' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'?' => {
                // Skip `?<digit>` placeholders.
                if i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                // Skip bare `?` placeholders (anything that doesn't match
                // the JSONB RHS shape — see jsonb_question_op_shape).
                if !jsonb_question_op_shape(bytes, i) {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                let next = bytes.get(i + 1).copied();
                let func = match next {
                    Some(b'|') => Some(("jsonb_exists_any", 2)),
                    Some(b'&') => Some(("jsonb_exists_all", 2)),
                    _ => Some(("jsonb_exists", 1)),
                };
                let Some((func_name, op_len)) = func else {
                    out.push(b as char);
                    i += 1;
                    continue;
                };
                let lhs_start = match find_jsonb_lhs_start(&out) {
                    Some(s) => s,
                    None => {
                        out.push(b as char);
                        i += 1;
                        continue;
                    }
                };
                let lhs = out[lhs_start..].trim_end().to_owned();
                if lhs.is_empty() {
                    out.push(b as char);
                    i += 1;
                    continue;
                }
                out.truncate(lhs_start);
                // Skip past the operator + any whitespace.
                let mut j = i + op_len;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                let (rhs_text, after_rhs) = match collect_jsonb_rhs(bytes, j, op_len > 1) {
                    Some(parts) => parts,
                    None => {
                        out.push_str(&lhs);
                        out.push(' ');
                        out.push(b as char);
                        i += 1;
                        continue;
                    }
                };
                out.push_str(&format!("{func_name}({lhs}, {rhs_text})"));
                i = after_rhs;
            }
            _ => {
                out.push(b as char);
                i += 1;
            }
        }
    }
    out
}

/// Find the byte offset within `prefix` where the JSONB LHS expression
/// most likely starts. Forward-scans `prefix` to mark string/comment
/// spans, then walks backward through the remaining "code" bytes,
/// tracking balanced parens / brackets and stopping at the nearest
/// outer SQL boundary (top-level comma, semicolon, or paren).
pub(crate) fn find_jsonb_lhs_start(prefix: &str) -> Option<usize> {
    let bytes = prefix.as_bytes();
    let mut is_code = vec![false; bytes.len()];
    let mut in_string: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(quote) = in_string {
            if b == quote {
                if i + 1 < bytes.len() && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' | b'`' => {
                in_string = Some(b);
                i += 1;
            }
            b'-' if i + 1 < bytes.len() && bytes[i + 1] == b'-' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
            }
            _ => {
                is_code[i] = true;
                i += 1;
            }
        }
    }

    let mut depth = 0i32;
    let mut idx = bytes.len();
    let stop_words: &[&[u8]] = &[
        b"select", b"where", b"from", b"group", b"order", b"having", b"limit", b"on", b"by",
        b"when", b"then", b"else", b"and", b"or", b"not", b"in", b"is", b"as", b"case", b"join",
        b"using", b"set", b"values",
    ];
    while idx > 0 {
        idx -= 1;
        if !is_code[idx] {
            // Inside a string/comment — skip over it as a unit.
            // Find the start of this contiguous non-code run.
            let mut start = idx;
            while start > 0 && !is_code[start - 1] {
                start -= 1;
            }
            idx = start;
            // If the non-code span is preceded by code we can keep
            // walking once we decrement past it.
            if idx == 0 {
                break;
            }
            continue;
        }
        let b = bytes[idx];
        match b {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                if depth == 0 {
                    return Some(idx + 1);
                }
                depth -= 1;
            }
            b',' | b';' if depth == 0 => return Some(idx + 1),
            // Stop at a keyword boundary (e.g. `SELECT … ? 'k'`).
            b' ' | b'\t' | b'\n' | b'\r' if depth == 0 => {
                // Peek backwards over consecutive whitespace.
                let mut k = idx;
                while k > 0 && matches!(bytes[k - 1], b' ' | b'\t' | b'\n' | b'\r') {
                    k -= 1;
                }
                // Identify the word ending at `k`.
                let word_end = k;
                let mut word_start = k;
                while word_start > 0
                    && is_code[word_start - 1]
                    && (bytes[word_start - 1].is_ascii_alphanumeric()
                        || bytes[word_start - 1] == b'_')
                {
                    word_start -= 1;
                }
                if word_end > word_start {
                    let lower: Vec<u8> = bytes[word_start..word_end]
                        .iter()
                        .map(|b| b.to_ascii_lowercase())
                        .collect();
                    if stop_words.iter().any(|w| *w == lower.as_slice()) {
                        return Some(word_end + 1);
                    }
                }
            }
            _ => {}
        }
    }
    Some(0)
}

/// Collect the right-hand side of a JSONB question-mark operator.
/// When `is_array` is true we expect `ARRAY[...]` and unwrap its
/// contents; otherwise we expect a single scalar expression (literal
/// or parenthesised). Returns `(rendered_args, idx_after_rhs)`.
pub(crate) fn collect_jsonb_rhs(
    bytes: &[u8],
    start: usize,
    is_array: bool,
) -> Option<(String, usize)> {
    if is_array {
        // Match `ARRAY[ ... ]`.
        let prefix = b"ARRAY[";
        if start + prefix.len() > bytes.len() {
            return None;
        }
        let upper: Vec<u8> = bytes[start..start + prefix.len()]
            .iter()
            .map(|b| b.to_ascii_uppercase())
            .collect();
        if upper.as_slice() != prefix {
            return None;
        }
        let mut j = start + prefix.len();
        let body_start = j;
        let mut depth = 1i32;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                b'\'' => {
                    j += 1;
                    while j < bytes.len() {
                        if bytes[j] == b'\'' {
                            if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                                j += 2;
                                continue;
                            }
                            break;
                        }
                        j += 1;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if depth != 0 {
            return None;
        }
        let body = std::str::from_utf8(&bytes[body_start..j - 1]).ok()?.trim();
        Some((body.to_owned(), j))
    } else {
        // Single expression: literal, identifier, or balanced
        // parenthesised expression.
        let mut j = start;
        if j >= bytes.len() {
            return None;
        }
        let first = bytes[j];
        if first == b'(' {
            let mut depth = 1i32;
            j += 1;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?.trim();
            Some((text.to_owned(), j))
        } else if first == b'\'' {
            j += 1;
            while j < bytes.len() {
                if bytes[j] == b'\'' {
                    if j + 1 < bytes.len() && bytes[j + 1] == b'\'' {
                        j += 2;
                        continue;
                    }
                    j += 1;
                    break;
                }
                j += 1;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?;
            Some((text.to_owned(), j))
        } else {
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_' || bytes[j] == b'.')
            {
                j += 1;
            }
            if j == start {
                return None;
            }
            let text = std::str::from_utf8(&bytes[start..j]).ok()?;
            Some((text.to_owned(), j))
        }
    }
}

pub(crate) fn rewrite_strict_without_rowid_combo(sql: &str) -> String {
    if !contains_ignore_ascii_case(sql, b"strict")
        || !contains_ignore_ascii_case(sql, b"without rowid")
    {
        return sql.to_owned();
    }

    let bytes = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut last = 0usize;
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' | b'`' => {
                i = scan_quoted(bytes, i, bytes[i]);
                continue;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            _ => {}
        }

        if let Some(end) = parse_strict_without_rowid_options(bytes, i) {
            out.push_str(&sql[last..i]);
            out.push_str("WITHOUT ROWID STRICT");
            last = end;
            i = end;
            changed = true;
            continue;
        }
        if let Some(end) = parse_without_rowid_strict_options(bytes, i) {
            out.push_str(&sql[last..i]);
            out.push_str("WITHOUT ROWID STRICT");
            last = end;
            i = end;
            changed = true;
            continue;
        }

        i += 1;
    }

    if changed {
        out.push_str(&sql[last..]);
        out
    } else {
        sql.to_owned()
    }
}

pub(crate) fn parse_strict_without_rowid_options(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut i = parse_strict_option(bytes, pos)?;
    i = skip_ascii_ws(bytes, i);
    if bytes.get(i) != Some(&b',') {
        return None;
    }
    i = skip_ascii_ws(bytes, i + 1);
    parse_without_rowid_option(bytes, i)
}

pub(crate) fn parse_without_rowid_strict_options(bytes: &[u8], pos: usize) -> Option<usize> {
    let mut i = parse_without_rowid_option(bytes, pos)?;
    i = skip_ascii_ws(bytes, i);
    if bytes.get(i) != Some(&b',') {
        return None;
    }
    i = skip_ascii_ws(bytes, i + 1);
    parse_strict_option(bytes, i)
}

pub(crate) fn parse_strict_option(bytes: &[u8], pos: usize) -> Option<usize> {
    if matches_keyword_ci_bounded(bytes, pos, b"STRICT") {
        Some(pos + b"STRICT".len())
    } else {
        None
    }
}

pub(crate) fn parse_without_rowid_option(bytes: &[u8], pos: usize) -> Option<usize> {
    if !matches_keyword_ci_bounded(bytes, pos, b"WITHOUT") {
        return None;
    }
    let after_without = pos + b"WITHOUT".len();
    let rowid_pos = skip_ascii_ws(bytes, after_without);
    if rowid_pos == after_without || !matches_keyword_ci_bounded(bytes, rowid_pos, b"ROWID") {
        return None;
    }
    Some(rowid_pos + b"ROWID".len())
}

pub(crate) fn matches_keyword_ci_bounded(bytes: &[u8], pos: usize, keyword: &[u8]) -> bool {
    matches_keyword_ci(bytes, pos, keyword)
        && pos
            .checked_sub(1)
            .is_none_or(|prev| !is_word_char(bytes[prev]))
        && bytes
            .get(pos + keyword.len())
            .is_none_or(|next| !is_word_char(*next))
}

pub(crate) fn skip_ascii_ws(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}
