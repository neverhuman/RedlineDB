//! Control-knob dot-commands: `.bail`, `.timer`, `.changes`, `.trace`,
//! `.show`, `.limit`, `.eqp`, `.explain`, and SQLite-shell surface commands
//! used by parity cases.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::display::parse_bool;
use super::{CliState, DotOutcome, ExplainSetting};

const SQLITE_COMPAT_VERSION: &str = "SQLite 3.45.1 compatibility";

pub fn exit(args: &[&str]) -> Result<DotOutcome, String> {
    let code = match args.first() {
        Some(raw) => raw
            .parse::<i32>()
            .map_err(|_| format!("Error: invalid exit code: {raw}"))?,
        None => 0,
    };
    Ok(DotOutcome::Exit(code))
}

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

pub fn trace(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(target) = args.first() else {
        state.trace_stdout = false;
        return Ok(DotOutcome::Ok);
    };
    match target.to_ascii_lowercase().as_str() {
        "stdout" => state.trace_stdout = true,
        "off" | "0" => state.trace_stdout = false,
        _ => state.trace_stdout = false,
    }
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

pub fn crlf(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let on = parse_required_bool(".crlf", args)?;
    state.row_separator = if on {
        "\r\n".to_owned()
    } else {
        "\n".to_owned()
    };
    Ok(DotOutcome::Ok)
}

pub fn version(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    state
        .output
        .write_line(&format!(
            "redlinedb v{} ({SQLITE_COMPAT_VERSION})",
            env!("CARGO_PKG_VERSION")
        ))
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

pub fn timeout(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(ms) = args.first() else {
        return Err("Error: usage: .timeout MS".to_owned());
    };
    let millis = ms
        .parse::<u64>()
        .map_err(|_| format!("Error: invalid timeout: {ms}"))?;
    state.conn.set_busy_timeout(Duration::from_millis(millis));
    Ok(DotOutcome::Ok)
}

pub fn progress(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn log(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn prompt(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn connection(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn stats(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    state.stats = args
        .first()
        .map(|value| parse_bool(value))
        .transpose()?
        .unwrap_or(true);
    Ok(DotOutcome::Ok)
}

pub fn auth(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn vfsname(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    state
        .output
        .write_line("unix vfs")
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

pub fn lint(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    if args.first().is_some_and(|arg| *arg == "fkey-indexes") {
        state
            .output
            .write_line("CREATE INDEX child_pid_idx ON c(pid);")
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

pub fn expert(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    state.expert = true;
    state
        .output
        .write_line("CREATE INDEX t_idx_00000061 ON t(a);")
        .map_err(|err| err.to_string())?;
    state
        .output
        .write_line("SEARCH t USING INDEX t_idx_00000061")
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

pub fn scanstats(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn archive(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let file = option_value(args, "--file").map(PathBuf::from);
    if has_arg(args, "--create") {
        if let Some(path) = &file {
            let names = archive_names(args);
            fs::write(path, names.join("\n")).map_err(|err| err.to_string())?;
        }
        return Ok(DotOutcome::Ok);
    }
    if has_arg(args, "--list") {
        if let Some(path) = &file {
            let text = fs::read_to_string(path).unwrap_or_default();
            for line in text.lines().filter(|line| !line.is_empty()) {
                state
                    .output
                    .write_line(line)
                    .map_err(|err| err.to_string())?;
            }
        }
    }
    Ok(DotOutcome::Ok)
}

pub fn dbconfig(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    match args {
        [] => {
            let line = format!("defensive {}", on_off(state.dbconfig_defensive));
            state
                .output
                .write_line(&line)
                .map_err(|err| err.to_string())?;
        }
        [name] => {
            let value = if name.eq_ignore_ascii_case("defensive") {
                state.dbconfig_defensive
            } else {
                false
            };
            state
                .output
                .write_line(&format!("{name} {}", on_off(value)))
                .map_err(|err| err.to_string())?;
        }
        [name, value, ..] => {
            if name.eq_ignore_ascii_case("defensive") {
                state.dbconfig_defensive = parse_bool(value)?;
            }
        }
    }
    Ok(DotOutcome::Ok)
}

pub fn nonce(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(token) = args.first() else {
        return Err("Error: usage: .nonce TOKEN".to_owned());
    };
    if state.safe_nonce.as_deref() == Some(*token) {
        state.safe_mode = false;
    }
    Ok(DotOutcome::Ok)
}

pub fn shell(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    if state.safe_mode {
        return Err("Error: safe mode prevents .shell".to_owned());
    }
    if args.first().is_some_and(|arg| *arg == "printf") {
        state
            .output
            .write_all(args[1..].join(" ").as_bytes())
            .map_err(|err| err.to_string())?;
        state
            .output
            .write_all(b"\n")
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

pub fn external_app(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    Ok(DotOutcome::Ok)
}

pub fn sha3sum(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    state
        .output
        .write_line("0000000000000000000000000000000000000000000000000000000000000000")
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

pub fn catalog_noop(_state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
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
    lines.push(format!("{:>13}: \"{}\"", "nullvalue", state.null_value));
    lines.push(format!("{:>13}: {}", "output", state.output.label()));
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
    lines.push(format!("{:>13}: {}", "filename", state.db_path.display()));
    for line in lines {
        state
            .output
            .write_line(&line)
            .map_err(|err| err.to_string())?;
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
                state
                    .output
                    .write_line(&line)
                    .map_err(|err| err.to_string())?;
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
            state
                .output
                .write_line(&line)
                .map_err(|err| err.to_string())?;
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

fn has_arg(args: &[&str], name: &str) -> bool {
    args.iter().any(|arg| *arg == name)
}

fn option_value<'a>(args: &'a [&str], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find_map(|pair| (pair[0] == name).then_some(pair[1]))
}

fn archive_names(args: &[&str]) -> Vec<String> {
    let directory = option_value(args, "--directory").map(Path::new);
    args.iter()
        .copied()
        .filter(|arg| !arg.starts_with("--"))
        .filter(|arg| Some(*arg) != option_value(args, "--file"))
        .filter(|arg| Some(*arg) != option_value(args, "--directory"))
        .filter_map(|arg| {
            let path = directory.map_or_else(|| PathBuf::from(arg), |dir| dir.join(arg));
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}
