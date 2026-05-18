//! `.parameter` dot-command: SQLite-style named-parameter binding.
//!
//! Subcommands implemented:
//!   - `set NAME VALUE` — store a parameter (string value)
//!   - `unset NAME`     — remove a parameter
//!   - `init` / `clear` — drop the entire parameter map
//!   - `list`           — print stored parameters, one per line
//!
//! The REPL applies stored parameters to each prepared statement just
//! before stepping by scanning the SQL surface for `:name`/`@name`/`$name`
//! placeholders and binding the matching entry with `bind_named`. Values
//! are bound as text; SQLite's affinity-promotion rules handle the rest.

use super::{CliState, DotOutcome};

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
                    .write_line(&format!("{name}\t{value}"))
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
    let value = rest[1..].join(" ");
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
/// `name`. We strip a single leading sigil so the storage key matches what
/// `bind_named` expects (the sigil-prefixed form used in SQL).
fn normalize_param_name(input: &str) -> String {
    if let Some(rest) = input
        .strip_prefix(':')
        .or_else(|| input.strip_prefix('@'))
        .or_else(|| input.strip_prefix('$'))
    {
        format!(":{rest}")
    } else {
        format!(":{input}")
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_param_name;

    #[test]
    fn normalize_strips_sigil_and_canonicalises_to_colon() {
        assert_eq!(normalize_param_name(":foo"), ":foo");
        assert_eq!(normalize_param_name("@foo"), ":foo");
        assert_eq!(normalize_param_name("$foo"), ":foo");
        assert_eq!(normalize_param_name("foo"), ":foo");
    }
}
