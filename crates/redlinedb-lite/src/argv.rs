//! Hand-rolled argv parser for the lite shell.
//!
//! Mirrors the SQLite shell CLI surface on the small subset we support
//! without a `Database`. Anything outside that surface forces the caller
//! to fall through to the full `redlinedb` binary via execve.

/// Parsed argv. `unsupported` is true if we encountered a flag we don't
/// implement; the caller must fall through in that case.
pub struct ParsedArgv {
    pub help: bool,
    pub version: bool,
    pub csv: bool,
    pub separator: String,
    pub row_separator: String,
    pub null_value: String,
    /// Positional SQL strings (the bits that look like statements rather
    /// than filenames or flags). Preserves order.
    pub sql: Vec<String>,
    /// Optional database filename (`:memory:`, a path, or `None`).
    pub filename: Option<String>,
    /// True if argv contained a flag or shape we don't handle. The caller
    /// must execve the full binary.
    pub unsupported: bool,
}

impl ParsedArgv {
    fn new() -> Self {
        Self {
            help: false,
            version: false,
            csv: false,
            separator: "|".to_owned(),
            row_separator: "\n".to_owned(),
            null_value: String::new(),
            sql: Vec::new(),
            filename: None,
            unsupported: false,
        }
    }
}

pub fn parse(argv: &[String]) -> ParsedArgv {
    let mut out = ParsedArgv::new();
    let mut i = 1; // skip argv[0]
    while i < argv.len() {
        let raw = &argv[i];
        // Normalize to a canonical long form. sqlite shell accepts both
        // -batch and --batch; we treat them the same.
        let long: String = if raw.starts_with("--") {
            raw.clone()
        } else if raw.starts_with('-') && raw.len() > 2 {
            format!("-{}", raw)
        } else {
            raw.clone()
        };

        match long.as_str() {
            "--help" | "-h" => out.help = true,
            "--version" | "-v" => out.version = true,
            "--csv" => out.csv = true,
            "--separator" => {
                i += 1;
                if i >= argv.len() {
                    out.unsupported = true;
                    return out;
                }
                out.separator = argv[i].clone();
            }
            "--newline" => {
                i += 1;
                if i >= argv.len() {
                    out.unsupported = true;
                    return out;
                }
                out.row_separator = argv[i].clone();
            }
            "--nullvalue" => {
                i += 1;
                if i >= argv.len() {
                    out.unsupported = true;
                    return out;
                }
                out.null_value = argv[i].clone();
            }
            "--batch" => {
                // batch mode is the only mode we offer; accept and ignore.
            }
            _ if raw.starts_with('-') => {
                // Any other flag we don't know -> fall through.
                out.unsupported = true;
                return out;
            }
            _ => {
                // Positional. First non-flag is the filename; subsequent
                // ones are SQL. `:memory:` is treated as filename when in
                // first slot.
                if out.filename.is_none() {
                    out.filename = Some(raw.clone());
                } else {
                    out.sql.push(raw.clone());
                }
            }
        }
        i += 1;
    }
    out
}
