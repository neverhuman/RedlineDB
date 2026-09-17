//! Output helpers. Kept tiny: lock stdout once, write, flush, exit.

use std::io::{self, Write};

pub fn write_and_exit(buf: &str, code: i32) -> ! {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(buf.as_bytes());
    let _ = handle.flush();
    std::process::exit(code);
}

pub fn print_help() {
    let stdout = io::stdout();
    let mut h = stdout.lock();
    let _ = h.write_all(LITE_HELP.as_bytes());
    let _ = h.flush();
}

pub fn print_version() {
    let stdout = io::stdout();
    let mut h = stdout.lock();
    let line = format!(
        "redlinedb v{} (SQLite 3.45.1 compatibility)\n",
        env!("CARGO_PKG_VERSION")
    );
    let _ = h.write_all(line.as_bytes());
    let _ = h.flush();
}

// Trimmed help. The full CLI prints a much longer banner; for the lite
// pre-filter we mirror only the supported surface and direct users to
// `redlinedb --help` for the rest.
const LITE_HELP: &str = "Usage: redlinedb-lite [OPTIONS] [FILENAME] [SQL]\n\
\n\
Lite pre-filter for redlinedb. Handles trivially-safe shell commands\n\
(.help / .version / .print / .show / .exit / .quit and fromless scalar\n\
SELECTs) without opening the engine. Everything else is handed off to\n\
the full `redlinedb` binary.\n\
\n\
Options:\n\
  -help                   Show this message\n\
  -version                Print version and exit\n\
  -batch                  Force batch mode (accepted; lite is always batch)\n\
  -csv                    CSV output (passthrough only)\n\
  -separator SEP          Column separator (default \"|\")\n\
  -newline SEP            Row separator (default \"\\n\")\n\
  -nullvalue STR          Text shown for NULL\n\
";
