//! Control-knob dot-commands: `.bail`, `.timer`, `.changes`, `.echo`,
//! `.show`, `.limit`, `.eqp`, `.explain`.

use super::{CliState, DotOutcome, ExplainSetting};
use super::display::parse_bool;

pub fn bail(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.bail = parse_required_bool(".bail", args)?;
    Ok(DotOutcome::Ok)
}

pub fn timer(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.timer = parse_required_bool(".timer", args)?;
    Ok(DotOutcome::Ok)
}

pub fn changes(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.changes = parse_required_bool(".changes", args)?;
    Ok(DotOutcome::Ok)
}

pub fn echo(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.echo = parse_required_bool(".echo", args)?;
    Ok(DotOutcome::Ok)
}

pub fn eqp(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.eqp = parse_required_bool(".eqp", args)?;
    Ok(DotOutcome::Ok)
}

pub fn explain(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(arg) = args.first() else {
        return Err("Error: usage: .explain on|off|auto".to_owned());
    };
    state.explain = match arg.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => ExplainSetting::On,
        "off" | "0" | "false" => ExplainSetting::Off,
        "auto" => ExplainSetting::Auto,
        other => {
            return Err(format!(
                "Error: .explain expects on|off|auto (got: {other})"
            ));
        }
    };
    Ok(DotOutcome::Ok)
}

/// `.show` — dump the current configuration to the active output.
pub fn show(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    let mut lines = Vec::new();
    lines.push(format!("{:>13}: {}", "echo", on_off(state.echo)));
    lines.push(format!("{:>13}: {}", "eqp", on_off(state.eqp)));
    let explain_label = match state.explain {
        ExplainSetting::On => "on",
        ExplainSetting::Off => "off",
        ExplainSetting::Auto => "auto",
    };
    lines.push(format!("{:>13}: {}", "explain", explain_label));
    lines.push(format!("{:>13}: {}", "headers", on_off(state.show_header)));
    lines.push(format!("{:>13}: {}", "mode", state.mode.name()));
    lines.push(format!(
        "{:>13}: \"{}\"",
        "nullvalue", state.null_value
    ));
    lines.push(format!(
        "{:>13}: {}",
        "output",
        state.output.label()
    ));
    lines.push(format!(
        "{:>13}: \"{}\"",
        "separator",
        escape(&state.separator)
    ));
    let widths_disp = if state.widths.is_empty() {
        "(none)".to_owned()
    } else {
        state
            .widths
            .iter()
            .map(|w| w.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    lines.push(format!("{:>13}: {}", "width", widths_disp));
    lines.push(format!(
        "{:>13}: {}",
        "filename",
        state.db_path.display()
    ));
    for line in lines {
        state.output.write_line(&line).map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.limit ?OPT? ?N?` — inspect or set a named limit.
///
/// With no arguments, prints all currently configured limits. With one
/// argument, prints the value for that limit. With two arguments, sets the
/// limit. SQLite limit names recognised: `length`, `sql_length`, `column`,
/// `expr_depth`, `compound_select`, `vdbe_op`, `function_arg`, `attached`,
/// `like_pattern_length`, `variable_number`, `trigger_depth`, `worker_threads`.
pub fn limit(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    match args.len() {
        0 => {
            for (name, value) in &state.limits {
                let line = format!("{name:>20} {value}");
                state.output.write_line(&line).map_err(|err| err.to_string())?;
            }
            Ok(DotOutcome::Ok)
        }
        1 => {
            let key = args[0].to_ascii_lowercase();
            let value = state
                .limits
                .iter()
                .find_map(|(k, v)| (k == &key).then_some(*v))
                .unwrap_or(0);
            let line = format!("{key:>20} {value}");
            state.output.write_line(&line).map_err(|err| err.to_string())?;
            Ok(DotOutcome::Ok)
        }
        _ => {
            let key = args[0].to_ascii_lowercase();
            let value: i64 = args[1]
                .parse()
                .map_err(|_| format!("Error: invalid limit value: {}", args[1]))?;
            if let Some(slot) = state.limits.iter_mut().find(|(k, _)| k == &key) {
                slot.1 = value;
            } else {
                state.limits.push((key, value));
            }
            Ok(DotOutcome::Ok)
        }
    }
}

fn parse_required_bool(cmd: &str, args: &[&str]) -> Result<bool, String> {
    match args.first() {
        Some(value) => parse_bool(value),
        None => Err(format!("Error: usage: {cmd} on|off")),
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}
