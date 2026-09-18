//! CREATE VIRTUAL TABLE prefix classifier.
#![allow(dead_code)]

pub(crate) fn starts_with_create_virtual_table(stmt: &str) -> bool {
    let mut rest = stmt.as_bytes();
    matches!(
        (
            next_sql_keyword(&mut rest),
            next_sql_keyword(&mut rest),
            next_sql_keyword(&mut rest),
        ),
        (Some(create), Some(virtual_kw), Some(table))
            if create.eq_ignore_ascii_case(b"create")
                && virtual_kw.eq_ignore_ascii_case(b"virtual")
                && table.eq_ignore_ascii_case(b"table")
    )
}

/// Return the next unquoted SQL word after whitespace and comments.
///
/// This is intentionally a small prefix lexer rather than a SQL rewriter:
/// the caller only needs to classify `CREATE VIRTUAL TABLE` before the main
/// parser can construct a compatibility template. Punctuation, literals, and
/// quoted identifiers stop classification instead of being guessed through.
pub(crate) fn next_sql_keyword<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    loop {
        while rest.first().is_some_and(u8::is_ascii_whitespace) {
            *rest = &rest[1..];
        }

        if rest.starts_with(b"--") {
            let end = rest
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(rest.len());
            *rest = &rest[end..];
            continue;
        }
        if rest.starts_with(b"/*") {
            let Some(end) = rest[2..].windows(2).position(|window| window == b"*/") else {
                *rest = &[];
                return None;
            };
            *rest = &rest[end + 4..];
            continue;
        }
        break;
    }

    let len = rest
        .iter()
        .position(|byte| !is_sql_word_byte(*byte))
        .unwrap_or(rest.len());
    if len == 0 {
        return None;
    }
    let (word, tail) = rest.split_at(len);
    *rest = tail;
    Some(word)
}

pub(crate) fn is_sql_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || !byte.is_ascii()
}
