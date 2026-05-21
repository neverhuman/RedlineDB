//! `.parameter` dot-command: SQLite-style named-parameter binding.
//!
//! Subcommands implemented:
//!   - `set NAME VALUE` — store a typed parameter value
//!   - `unset NAME`     — remove a parameter
//!   - `init` / `clear` — drop the entire parameter map
//!   - `list`           — print stored parameters, one per line
//!
//! The REPL applies stored parameters to each prepared statement just
//! before stepping by scanning the SQL surface for `:name`/`@name`/`$name`
//! placeholders and binding the matching entry with `bind_named`.

use std::sync::Arc;

use super::{CliState, DotOutcome};

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl ParameterValue {
    pub fn to_redlinedb_value(&self) -> redlinedb::Value {
        match self {
            Self::Null => redlinedb::Value::Null,
            Self::Integer(value) => redlinedb::Value::Integer(*value),
            Self::Real(value) => redlinedb::Value::Real(*value),
            Self::Text(value) => redlinedb::Value::Text(Arc::from(value.as_str())),
            Self::Blob(value) => redlinedb::Value::Blob(Arc::from(value.as_slice())),
        }
    }

    fn display_value(&self) -> String {
        match self {
            Self::Null => "NULL".to_owned(),
            Self::Integer(value) => value.to_string(),
            Self::Real(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Blob(value) => {
                let mut out = String::from("X'");
                for byte in value {
                    out.push_str(&format!("{byte:02X}"));
                }
                out.push('\'');
                out
            }
        }
    }
}

pub fn parameter(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some((sub, rest)) = args.split_first() else {
        return Err("Error: usage: .parameter set|unset|init|clear|list ...".to_owned());
    };
    match sub.to_ascii_lowercase().as_str() {
        "set" => parameter_set(state, rest),
        "unset" => parameter_unset(state, rest),
        "init" | "clear" => {
            state.params.clear();
            Ok(DotOutcome::Ok)
        }
        "list" => {
            for (name, value) in &state.params {
                state
                    .output
                    .write_line(&format!("{name}\t{}", value.display_value()))
                    .map_err(|err| err.to_string())?;
            }
            Ok(DotOutcome::Ok)
        }
        other => Err(format!(
            "Error: unknown .parameter subcommand: {other}; expected set|unset|init|clear|list"
        )),
    }
}

fn parameter_set(state: &mut CliState, rest: &[&str]) -> Result<DotOutcome, String> {
    if rest.len() < 2 {
        return Err("Error: usage: .parameter set NAME VALUE".to_owned());
    }
    let name = normalize_param_name(rest[0]);
    let value = parse_parameter_value(&rest[1..].join(" "));
    state.params.insert(name, value);
    Ok(DotOutcome::Ok)
}

fn parameter_unset(state: &mut CliState, rest: &[&str]) -> Result<DotOutcome, String> {
    let Some(name) = rest.first() else {
        return Err("Error: usage: .parameter unset NAME".to_owned());
    };
    state.params.remove(&normalize_param_name(name));
    Ok(DotOutcome::Ok)
}

/// SQLite's `.parameter set` accepts `:name`, `@name`, `$name`, or bare
/// `name`. Sigil-prefixed names are preserved so `.parameter list` and
/// `bind_named` match the SQL text exactly; bare names default to `:name`.
fn normalize_param_name(input: &str) -> String {
    if input.starts_with(':') || input.starts_with('@') || input.starts_with('$') {
        input.to_owned()
    } else {
        format!(":{input}")
    }
}

fn parse_parameter_value(input: &str) -> ParameterValue {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return ParameterValue::Null;
    }
    if let Some(blob) = parse_blob_literal(trimmed) {
        return ParameterValue::Blob(blob);
    }
    if let Ok(value) = trimmed.parse::<i64>() {
        return ParameterValue::Integer(value);
    }
    if looks_real(trimmed)
        && let Ok(value) = trimmed.parse::<f64>()
    {
        return ParameterValue::Real(value);
    }
    ParameterValue::Text(trimmed.to_owned())
}

fn looks_real(value: &str) -> bool {
    value.contains('.') || value.contains('e') || value.contains('E')
}

fn parse_blob_literal(value: &str) -> Option<Vec<u8>> {
    let body = value
        .strip_prefix("x'")
        .or_else(|| value.strip_prefix("X'"))?
        .strip_suffix('\'')?;
    if body.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(body.len() / 2);
    let raw = body.as_bytes();
    for pair in raw.chunks_exact(2) {
        let hi = hex_value(pair[0])?;
        let lo = hex_value(pair[1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ParameterValue, normalize_param_name, parse_parameter_value};

    #[test]
    fn normalize_strips_sigil_and_canonicalises_to_colon() {
        assert_eq!(normalize_param_name(":foo"), ":foo");
        assert_eq!(normalize_param_name("@foo"), "@foo");
        assert_eq!(normalize_param_name("$foo"), "$foo");
        assert_eq!(normalize_param_name("foo"), ":foo");
    }

    #[test]
    fn parse_parameter_values_preserves_storage_class() {
        assert_eq!(parse_parameter_value("7"), ParameterValue::Integer(7));
        assert_eq!(parse_parameter_value("7.5"), ParameterValue::Real(7.5));
        assert_eq!(parse_parameter_value("NULL"), ParameterValue::Null);
        assert_eq!(
            parse_parameter_value("text"),
            ParameterValue::Text("text".to_owned())
        );
    }
}
