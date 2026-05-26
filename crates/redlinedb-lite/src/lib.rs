//! redlinedb-lite: stripped pre-filter binary.
//!
//! See `Cargo.toml` for the dependency policy. The library entry point
//! `run` is exposed so integration tests can drive the same code path
//! that the `main` binary uses.

mod argv;
mod output;
mod shellzero;

use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Entry point. Returns the process exit code. The binary calls
/// `std::process::exit(run(...))` so tests can inspect the integer.
pub fn run<I>(raw_argv: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let argv: Vec<String> = raw_argv
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();

    let parsed = argv::parse(&argv);

    if parsed.unsupported {
        return exec_full(&argv);
    }

    if parsed.help {
        output::print_help();
        return 0;
    }
    if parsed.version {
        output::print_version();
        return 0;
    }

    // Read stdin only if it's non-TTY and there's nothing positional.
    // If we have positional SQL, we never read stdin (matches sqlite).
    let stdin_buf: Option<String> = if parsed.sql.is_empty() && !io::stdin().is_terminal() {
        let mut s = String::new();
        if io::stdin().read_to_string(&mut s).is_err() {
            return exec_full(&argv);
        }
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    };

    let dispatch = shellzero::dispatch(shellzero::DispatchArgs {
        sql: &parsed.sql,
        stdin: stdin_buf.as_deref(),
        separator: &parsed.separator,
        row_separator: &parsed.row_separator,
        null_value: &parsed.null_value,
    });

    match dispatch {
        shellzero::Dispatch::Handled { out, code } => output::write_and_exit(&out, code),
        shellzero::Dispatch::Unsupported => exec_full_with_stdin(&argv, stdin_buf.as_deref()),
    }
}

/// Hand off to the full `redlinedb` binary via execve. Stdin is
/// already drained at the parent's end if we read it. When we haven't
/// read stdin (e.g. positional SQL only, or a TTY), we use execvp so
/// the child inherits our file descriptors directly with zero copying.
fn exec_full(argv: &[String]) -> i32 {
    exec_full_with_stdin(argv, None)
}

fn exec_full_with_stdin(argv: &[String], replay_stdin: Option<&str>) -> i32 {
    let full = locate_full_binary();
    let mut cmd = Command::new(&full);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }
    match replay_stdin {
        None => {
            // Zero-cost handoff: replace ourselves with the full binary.
            let err = cmd.exec();
            // Only reached on exec failure.
            eprintln!("redlinedb-lite: exec {full:?} failed: {err}");
            127
        }
        Some(buf) => {
            // We already drained stdin to decide whether to fall through.
            // Spawn instead of exec so we can replay the buffered bytes.
            cmd.stdin(std::process::Stdio::piped());
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("redlinedb-lite: spawn {full:?} failed: {e}");
                    return 127;
                }
            };
            if let Some(mut sin) = child.stdin.take() {
                use std::io::Write;
                let _ = sin.write_all(buf.as_bytes());
            }
            match child.wait() {
                Ok(status) => status.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("redlinedb-lite: wait failed: {e}");
                    1
                }
            }
        }
    }
}

/// Locate the full `redlinedb` binary. Strategy: env var override
/// (used by tests), then alongside our own exe, then PATH.
fn locate_full_binary() -> String {
    if let Ok(p) = std::env::var("REDLINEDB_LITE_FULL_BIN") {
        return p;
    }
    if let Ok(mut exe) = std::env::current_exe() {
        exe.pop();
        let candidate = exe.join("redlinedb");
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    "redlinedb".to_owned()
}
