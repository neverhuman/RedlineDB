//! Display-formatting dot-commands.

use super::{CliState, DotOutcome, OutputMode};

/// `.mode {csv|json|line|markdown|table|tabs|insert|column|html|quote|list}`
pub fn mode(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(token) = args.first() else {
        return Err("Error: usage: .mode MODE".to_owned());
    };
    let Some(parsed) = OutputMode::parse(token) else {
        return Err(format!(
            "Error: mode should be one of: list csv json line markdown table tabs insert column html quote (got: {token})"
        ));
    };
    state.mode = parsed;
    state.separator = parsed.default_separator().to_owned();
    Ok(DotOutcome::Ok)
}

/// `.headers ON|OFF`
pub fn headers(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(arg) = args.first() else {
        return Err("Error: usage: .headers on|off".to_owned());
    };
    state.show_header = parse_bool(arg)?;
    Ok(DotOutcome::Ok)
}

/// `.width N1 N2 N3 ...` — overrides column widths for `column` mode.
pub fn width(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let mut widths = Vec::with_capacity(args.len());
    for token in args {
        let value: i64 = token
            .parse()
            .map_err(|_| format!("Error: invalid width: {token}"))?;
        widths.push(value.unsigned_abs() as usize);
    }
    state.widths = widths;
    Ok(DotOutcome::Ok)
}

/// `.nullvalue STRING`
pub fn nullvalue(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.null_value = args.join(" ");
    Ok(DotOutcome::Ok)
}

/// `.separator COL [ROW]` — only the column separator is applied; the row
/// separator is accepted but ignored to match SQLite when row separators are
/// irrelevant for the active output mode.
pub fn separator(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(col) = args.first() else {
        return Err("Error: usage: .separator COL ?ROW?".to_owned());
    };
    state.separator = unescape(col);
    Ok(DotOutcome::Ok)
}

pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Ok(true),
        "off" | "0" | "false" | "no" => Ok(false),
        other => Err(format!("Error: expected on or off (got: {other})")),
    }
}

fn unescape(value: &str) -> String {
    // SQLite recognises a small set of escapes inside the separator string.
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_basic() {
        assert_eq!(unescape("|"), "|");
        assert_eq!(unescape("\\t"), "\t");
        assert_eq!(unescape("\\\\"), "\\");
        assert_eq!(unescape("a\\nb"), "a\nb");
    }
}
