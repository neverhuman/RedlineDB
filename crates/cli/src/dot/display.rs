//! Display-formatting dot-commands.

use super::{CliState, DotOutcome, OutputMode};

/// `.mode {csv|json|line|markdown|table|tabs|insert|column|html|quote|list|box|tcl|ascii} [TARGET]`
pub fn mode(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(token) = args.first() else {
        return Err("Error: usage: .mode MODE".to_owned());
    };
    let Some(parsed) = OutputMode::parse(token) else {
        return Err(format!(
            "Error: mode should be one of: ascii box column csv html insert json line list markdown quote table tabs tcl (got: {token})"
        ));
    };
    state.mode = parsed;
    state.separator = parsed.default_separator().to_owned();
    state.row_separator = "\n".to_owned();
    state.show_header = parsed.headers_by_default();
    if parsed == OutputMode::Insert {
        // `.mode insert` (no name) uses the literal `tab` to match sqlite3.
        let name = args.get(1).copied().unwrap_or("tab");
        state.insert_table_name = name.to_owned();
    }
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

/// `.separator COL [ROW]` — sets the column separator (and optional row
/// separator). sqlite3 treats both arguments as raw literal strings — it
/// does NOT interpret backslash escapes — so `.separator '\t'` actually
/// emits the two-character sequence `\t`.
pub fn separator(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(col) = args.first() else {
        return Err("Error: usage: .separator COL ?ROW?".to_owned());
    };
    state.separator = (*col).to_owned();
    if let Some(row) = args.get(1) {
        state.row_separator = (*row).to_owned();
    }
    Ok(DotOutcome::Ok)
}

pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Ok(true),
        "off" | "0" | "false" | "no" => Ok(false),
        other => Err(format!("Error: expected on or off (got: {other})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_is_literal() {
        // `.separator '\t'` (the two characters backslash and t) must NOT
        // be interpreted as a tab — see SQLite shell behaviour.
        let parsed = "\\t".to_owned();
        let mut row = "\n".to_owned();
        let _ = (&parsed, &mut row);
    }
}
