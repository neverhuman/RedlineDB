/// Split `sql` into `(head, tail)` where `head` is the first statement
/// (including its trailing `;` if present) and `tail` is the remainder of the
/// input. `tail` is a byte slice of the original `sql`, with no leading
/// whitespace stripped - this matches SQLite's `pzTail` contract where the
/// tail pointer must reference into the caller's original buffer.
///
/// The split is "string-aware": semicolons inside `'...'`, `"..."`, or
/// `[...]` (SQLite bracket-quoting form) and inside `--` line comments or
/// `/* ... */` block comments are not considered terminators. Doubled quote
/// characters inside a string are treated as escapes.
///
/// If `sql` contains no terminating semicolon, the entire string is the
/// `head` and `tail` is empty. If `sql` is purely whitespace/comments, both
/// `head` and `tail` are returned trimmed appropriately.
pub fn split_first_statement(sql: &str) -> (&str, &str) {
    let split = split_first_statement_state(sql);
    (split.head, split.tail)
}

/// True when `sql` contains a complete first statement terminated by a
/// top-level semicolon. Semicolons inside strings, comments, and trigger bodies
/// do not count as terminators.
pub fn first_statement_complete(sql: &str) -> bool {
    split_first_statement_state(sql).terminated
}

struct StatementSplit<'a> {
    head: &'a str,
    tail: &'a str,
    terminated: bool,
}

fn split_first_statement_state(sql: &str) -> StatementSplit<'_> {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let len = bytes.len();
    let mut in_string: Option<u8> = None;
    // Lane A5-triggers: `CREATE TRIGGER ... BEGIN ... END` bodies contain
    // statement-terminating semicolons that must not split the outer
    // statement. We track a balanced BEGIN/END nesting depth (matched
    // case-insensitively on word boundaries) and only honour `;` at
    // depth 0. We only treat `BEGIN` as a block opener when the current
    // statement is a `CREATE TRIGGER`; bare `BEGIN [TRANSACTION]` and
    // `BEGIN IMMEDIATE` outside a trigger context must still split.
    let mut block_depth = 0usize;
    let mut in_trigger = false;
    while i < len {
        let b = bytes[i];
        if let Some(quote) = in_string {
            // Inside a string literal: handle escaped quote (doubled quote).
            if b == quote {
                if i + 1 < len && bytes[i + 1] == quote {
                    i += 2;
                    continue;
                }
                in_string = None;
                i += 1;
                continue;
            }
            // SQLite-style `[...]` bracket quoting closes on `]`.
            if quote == b'[' && b == b']' {
                in_string = None;
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_string = Some(b);
                i += 1;
            }
            b'[' => {
                in_string = Some(b'[');
                i += 1;
            }
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                // Line comment until \n or EOF.
                i += 2;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                // Block comment until */
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
            }
            b';' if block_depth == 0 => {
                let head_end = i + 1;
                return StatementSplit {
                    head: &sql[..head_end],
                    tail: &sql[head_end..],
                    terminated: true,
                };
            }
            b';' => {
                i += 1;
            }
            // A30: first-byte fast-reject for keyword scanning. The TRIGGER /
            // BEGIN / END word-boundary check is the hot inner work for
            // every byte that ISN'T already matched by the quote / comment /
            // semicolon arms above. By predicating each keyword arm on its
            // expected first letter (`T`/`t`, `B`/`b`, `E`/`e`), the
            // overwhelming majority of bytes — alphanumerics not starting
            // those keywords — skip the function call entirely and fall
            // through to the `_` arm in a single byte compare. Prior code
            // always called `is_word_boundary_keyword` per byte.
            b'T' | b't' if is_word_boundary_keyword(bytes, i, b"TRIGGER") => {
                in_trigger = true;
                i += 7;
            }
            b'B' | b'b' if in_trigger && is_word_boundary_keyword(bytes, i, b"BEGIN") => {
                block_depth += 1;
                i += 5;
            }
            b'E' | b'e' if in_trigger && is_word_boundary_keyword(bytes, i, b"END") => {
                block_depth = block_depth.saturating_sub(1);
                if block_depth == 0 {
                    in_trigger = false;
                }
                i += 3;
            }
            _ => {
                i += 1;
            }
        }
    }
    StatementSplit {
        head: sql,
        tail: "",
        terminated: false,
    }
}

/// True if `bytes[i..]` starts with `kw` (case-insensitively) AND the
/// surrounding characters form a word boundary - i.e. the preceding
/// byte (if any) and the byte immediately after `kw` are not ASCII
/// alphanumerics or underscore.
fn is_word_boundary_keyword(bytes: &[u8], i: usize, kw: &[u8]) -> bool {
    if i + kw.len() > bytes.len() {
        return false;
    }
    for (offset, expected) in kw.iter().enumerate() {
        if !bytes[i + offset].eq_ignore_ascii_case(expected) {
            return false;
        }
    }
    if i > 0 && is_ident_byte(bytes[i - 1]) {
        return false;
    }
    if i + kw.len() < bytes.len() && is_ident_byte(bytes[i + kw.len()]) {
        return false;
    }
    true
}

pub(crate) fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// True if `sql` (after trimming whitespace and stripping comments) is empty.
/// Used to detect SQL that is entirely a comment block - `sqlite3_prepare_v2`
/// treats such input as a successful no-op (`out_stmt` becomes NULL).
pub fn is_blank_sql(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let len = bytes.len();
    while i < len {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b';' {
            i += 1;
            continue;
        }
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            continue;
        }
        return false;
    }
    true
}

/// Iterate over every top-level statement in `sql`, yielding `(head, tail)`
/// pairs analogous to repeatedly calling `split_first_statement`. Skips runs
/// of pure-whitespace/comment chunks. The yielded `head` slice is the SQL of
/// one statement (including its trailing `;` if any) and `tail` is the
/// remainder of the input after that statement.
///
/// Used by tests and by callers that want the full split up-front; runtime
/// `Connection::execute` walks the splitter incrementally so it can stop at
/// the first failing statement.
#[allow(dead_code)]
pub fn split_statements(sql: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = sql;
    while !rest.is_empty() {
        if is_blank_sql(rest) {
            break;
        }
        let (head, tail) = split_first_statement(rest);
        if head.is_empty() {
            break;
        }
        if !is_blank_sql(head) {
            out.push(head);
        }
        rest = tail;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{first_statement_complete, split_first_statement};

    #[test]
    fn split_ignores_semicolons_in_line_comments() {
        let sql = "select 1 -- ; still a comment\n; select 2;";
        let (head, tail) = split_first_statement(sql);
        assert_eq!(head, "select 1 -- ; still a comment\n;");
        assert_eq!(tail, " select 2;");
        assert!(first_statement_complete(sql));
    }

    #[test]
    fn split_ignores_semicolons_in_block_comments() {
        let sql = "select 1 /* ; still a comment */; select 2;";
        let (head, tail) = split_first_statement(sql);
        assert_eq!(head, "select 1 /* ; still a comment */;");
        assert_eq!(tail, " select 2;");
        assert!(first_statement_complete(sql));
    }

    #[test]
    fn split_ignores_semicolons_in_quoted_strings() {
        let sql = "select ';not a terminator'; select 2;";
        let (head, tail) = split_first_statement(sql);
        assert_eq!(head, "select ';not a terminator';");
        assert_eq!(tail, " select 2;");
        assert!(first_statement_complete(sql));
    }

    #[test]
    fn split_ignores_semicolons_in_bracket_quoting() {
        let sql = "select [a; b] from t; select 2;";
        let (head, tail) = split_first_statement(sql);
        assert_eq!(head, "select [a; b] from t;");
        assert_eq!(tail, " select 2;");
        assert!(first_statement_complete(sql));
    }

    #[test]
    fn split_keeps_trigger_bodies_intact() {
        let sql = "CREATE TRIGGER trg AFTER INSERT ON t BEGIN INSERT INTO t VALUES (1); UPDATE t SET x = 2; END; select 2;";
        let (head, tail) = split_first_statement(sql);
        assert_eq!(
            head,
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN INSERT INTO t VALUES (1); UPDATE t SET x = 2; END;"
        );
        assert_eq!(tail, " select 2;");
        assert!(first_statement_complete(sql));
    }
}
