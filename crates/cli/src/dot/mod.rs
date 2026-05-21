//! Dot-command dispatch table for the SQLite-compatible shell.
//!
//! Each command group lives in its own module so individual files stay below
//! the 300-LOC ceiling. The public entry point is [`dispatch`], which is
//! called by `main` after observing a `.`-prefixed line at the start of an
//! input buffer.

use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

use redlinedb::{Connection, Database};

pub mod control;
pub mod display;
pub mod io_cmd;
pub mod parameter;
pub mod schema;

/// Output formatting modes accepted by `.mode` and the flag parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    List,
    Csv,
    Json,
    Line,
    Ascii,
    Markdown,
    Quote,
    Table,
    Tabs,
    Insert,
    Column,
    Html,
}

impl OutputMode {
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token.to_ascii_lowercase().as_str() {
            "list" => Self::List,
            "csv" => Self::Csv,
            "json" => Self::Json,
            "line" | "lines" => Self::Line,
            "ascii" => Self::Ascii,
            "markdown" => Self::Markdown,
            "quote" => Self::Quote,
            "table" | "box" => Self::Table,
            "tabs" => Self::Tabs,
            "insert" => Self::Insert,
            "column" => Self::Column,
            "html" => Self::Html,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Line => "line",
            Self::Ascii => "ascii",
            Self::Markdown => "markdown",
            Self::Quote => "quote",
            Self::Table => "table",
            Self::Tabs => "tabs",
            Self::Insert => "insert",
            Self::Column => "column",
            Self::Html => "html",
        }
    }

    pub fn default_separator(self) -> &'static str {
        match self {
            Self::Tabs => "\t",
            Self::Csv => ",",
            Self::Ascii => "\x1f",
            _ => "|",
        }
    }

    pub fn headers_by_default(self) -> bool {
        matches!(self, Self::Markdown | Self::Table)
    }
}

/// Mutable shell state that dot-commands read and update.
pub struct CliState {
    pub db: Database,
    pub conn: Connection,
    pub db_path: PathBuf,
    pub mode: OutputMode,
    pub separator: String,
    pub row_separator: String,
    pub show_header: bool,
    pub null_value: String,
    pub bail: bool,
    pub had_error: bool,
    pub timer: bool,
    pub changes: bool,
    pub echo: bool,
    pub trace_stdout: bool,
    pub eqp: bool,
    pub explain: ExplainSetting,
    pub stats: bool,
    pub expert: bool,
    pub escape_symbol: bool,
    pub dbconfig_defensive: bool,
    pub widths: Vec<usize>,
    pub limits: Vec<(String, i64)>,
    pub output: OutputTarget,
    pub snapshots: std::collections::BTreeMap<PathBuf, Database>,
    /// `.parameter set NAME VALUE` populates this map; the REPL binds
    /// these values to matching `:name`/`@name`/`$name` placeholders in
    /// any subsequent statement.
    pub params: std::collections::BTreeMap<String, parameter::ParameterValue>,
    /// `.once FILE` arms a one-shot output redirect for the next executed
    /// statement. The REPL takes ownership when running the next query and
    /// resets `output` afterwards.
    pub once: Option<PathBuf>,
    pub safe_mode: bool,
    pub safe_nonce: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainSetting {
    On,
    Off,
    Auto,
}

/// Sink for query output and `.print`.
pub enum OutputTarget {
    Stdout,
    Null,
    File { path: PathBuf, writer: File },
}

impl OutputTarget {
    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Self::Stdout => io::stdout().write_all(bytes),
            Self::Null => Ok(()),
            Self::File { writer, .. } => writer.write_all(bytes),
        }
    }

    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        self.write_all(line.as_bytes())?;
        self.write_all(b"\n")
    }

    pub fn label(&self) -> String {
        match self {
            Self::Stdout => "stdout".into(),
            Self::Null => "off".into(),
            Self::File { path, .. } => path.display().to_string(),
        }
    }
}

impl Write for OutputTarget {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Stdout => io::stdout().write(buf),
            Self::Null => Ok(buf.len()),
            Self::File { writer, .. } => writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stdout => io::stdout().flush(),
            Self::Null => Ok(()),
            Self::File { writer, .. } => writer.flush(),
        }
    }
}

impl CliState {
    pub fn new(
        db: Database,
        db_path: PathBuf,
        mode: OutputMode,
        separator: String,
        header: bool,
    ) -> Result<Self, String> {
        let conn = db.connect().map_err(|err| err.to_string())?;
        Ok(Self {
            db,
            conn,
            db_path,
            mode,
            separator,
            row_separator: "\n".to_owned(),
            show_header: header,
            null_value: String::new(),
            bail: false,
            had_error: false,
            timer: false,
            changes: false,
            echo: false,
            trace_stdout: false,
            eqp: false,
            explain: ExplainSetting::Auto,
            stats: false,
            expert: false,
            escape_symbol: false,
            dbconfig_defensive: false,
            widths: Vec::new(),
            limits: Vec::new(),
            output: OutputTarget::Stdout,
            snapshots: std::collections::BTreeMap::new(),
            params: std::collections::BTreeMap::new(),
            once: None,
            safe_mode: false,
            safe_nonce: None,
        })
    }

    pub fn reconnect(&mut self) -> Result<(), String> {
        self.conn = self.db.connect().map_err(|err| err.to_string())?;
        Ok(())
    }
}

/// Outcome of executing a dot-command.
pub enum DotOutcome {
    Ok,
    /// `.read FILE` must be executed by the REPL because executing SQL
    /// requires the caller's statement runner.
    ReadFile(PathBuf),
    /// `.exit` / `.quit` request termination of the REPL with the given
    /// exit code.
    Exit(i32),
}

/// Dispatch a single dot-command line. The `line` must include the leading
/// `.` and may include arguments separated by ASCII whitespace.
pub fn dispatch(state: &mut CliState, line: &str) -> Result<DotOutcome, String> {
    let line = line.trim();
    if !line.starts_with('.') {
        return Err(format!("not a dot-command: {line}"));
    }
    let parts = split_args(line);
    let Some((cmd, args)) = parts.split_first() else {
        return Err("empty dot-command".to_owned());
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match cmd.as_str() {
        ".exit" | ".quit" => control::exit(&args),
        ".help" => print_help(state),
        ".tables" => schema::tables(state, &args),
        ".schema" => schema::schema(state, &args),
        ".fullschema" => schema::fullschema(state, &args),
        ".indexes" | ".indices" => schema::indexes(state, &args),
        ".databases" => schema::databases(state, &args),
        ".dump" => io_cmd::dump(state, &args),
        ".backup" => io_cmd::backup(state, &args),
        ".clone" => io_cmd::clone_db(state, &args),
        ".save" => io_cmd::save(state, &args),
        ".restore" => io_cmd::restore(state, &args),
        ".open" => io_cmd::open(state, &args),
        ".cd" => io_cmd::cd(state, &args),
        ".output" => io_cmd::output(state, &args),
        ".once" => io_cmd::once(state, &args),
        ".parameter" | ".param" => parameter::parameter(state, &args),
        ".print" => io_cmd::print(state, &args),
        ".import" => io_cmd::import(state, &args),
        ".read" => io_cmd::read(state, &args),
        ".mode" => display::mode(state, &args),
        ".headers" | ".header" => display::headers(state, &args),
        ".width" | ".widths" => display::width(state, &args),
        ".nullvalue" => display::nullvalue(state, &args),
        ".separator" => display::separator(state, &args),
        ".bail" => control::bail(state, &args),
        ".timer" => control::timer(state, &args),
        ".changes" => control::changes(state, &args),
        ".trace" => control::trace(state, &args),
        ".version" => control::version(state, &args),
        ".timeout" => control::timeout(state, &args),
        ".progress" => control::progress(state, &args),
        ".log" => control::log(state, &args),
        ".prompt" => control::prompt(state, &args),
        ".dbconfig" => control::dbconfig(state, &args),
        ".connection" => control::connection(state, &args),
        ".nonce" => control::nonce(state, &args),
        ".shell" | ".system" => control::shell(state, &args),
        ".excel" | ".www" => control::external_app(state, &args),
        ".echo" => control::echo(state, &args),
        ".show" => control::show(state, &args),
        ".limit" => control::limit(state, &args),
        ".eqp" => control::eqp(state, &args),
        ".explain" => control::explain(state, &args),
        ".crlf" => control::crlf(state, &args),
        ".stats" => control::stats(state, &args),
        ".auth" => control::auth(state, &args),
        ".vfsname" => control::vfsname(state, &args),
        ".vfslist" | ".vfsinfo" => control::vfsname(state, &args),
        ".lint" => control::lint(state, &args),
        ".expert" => control::expert(state, &args),
        ".scanstats" => control::scanstats(state, &args),
        ".archive" => control::archive(state, &args),
        ".sha3sum" => control::sha3sum(state, &args),
        ".filectrl" | ".imposter" | ".intck" | ".session" | ".unmodule" | ".check" => {
            control::catalog_noop(state, &args)
        }
        ".dbinfo" => io_cmd::dbinfo(state, &args),
        ".dbtotxt" => io_cmd::dbtotxt(state, &args),
        ".recover" => io_cmd::recover(state, &args),
        other => Err(format!(
            "Error: unknown command or invalid arguments: \"{}\". Enter \".help\" for help",
            other.trim_start_matches('.')
        )),
    }
}

fn print_help(_state: &mut CliState) -> Result<DotOutcome, String> {
    let lines = [
        ".bail on|off            Stop after hitting an error",
        ".changes on|off         Show number of rows changed by SQL",
        ".crlf on|off            Toggle CRLF row separators",
        ".databases              List names and files of attached databases",
        ".dbinfo                 Show basic database information",
        ".dbtotxt                Render database contents as text",
        ".dump ?TABLE?           Render database content as SQL",
        ".echo on|off            Turn command echo on or off",
        ".eqp on|off             Enable / disable automatic EXPLAIN QUERY PLAN",
        ".exit                   Exit this program",
        ".expert                 Query planner advice (no-op here)",
        ".explain on|off|auto    Toggle EXPLAIN output mode",
        ".auth                   Show authorizer status (no-op here)",
        ".headers on|off         Turn display of headers on or off",
        ".help                   Show this message",
        ".import FILE TABLE      Import data from FILE into TABLE",
        ".lint                   Run lint checks (no-op here)",
        ".indexes ?TABLE?        List indexes",
        ".limit ?OPT? ?N?        Inspect or set SQLITE_LIMIT values",
        ".mode MODE              Set output mode",
        ".nullvalue STRING       Use STRING in place of NULL",
        ".once ?FILE?            Redirect the next single query to FILE",
        ".output ?FILE?          Send output to FILENAME or stdout",
        ".parameter CMD ...      set|unset|list|clear named SQL parameters",
        ".print STRING...        Print literal STRING",
        ".quit                   Exit this program",
        ".read FILENAME          Execute SQL from FILENAME",
        ".recover                Recover database contents as SQL",
        ".scanstats              Query scan statistics (no-op here)",
        ".restore FILE           Restore content of database from FILE",
        ".save FILE              Write in-memory database into FILE",
        ".stats                  Show database stats (no-op here)",
        ".schema ?TABLE?         Show CREATE statements",
        ".fullschema ?TABLE?     Show CREATE statements plus sqlite_master",
        ".vfsname                Show the active VFS name (no-op here)",
        ".separator SEP          Change separator string",
        ".show                   Show the current values for various settings",
        ".tables ?TABLE?         List names of tables matching PATTERN",
        ".timer on|off           Turn SQL timer on or off",
        ".width N1 N2 ...        Set minimum column widths for column mode",
    ];
    for line in lines {
        println!("{line}");
    }
    Ok(DotOutcome::Ok)
}

/// Split a dot-command line into tokens, honouring single/double quotes.
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut iter = line.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(ch) = iter.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            (None, '"') | (None, '\'') => quote = Some(ch),
            (None, c) => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_simple_tokens() {
        assert_eq!(split_args(".tables foo bar"), vec![".tables", "foo", "bar"]);
    }

    #[test]
    fn split_quoted_tokens() {
        assert_eq!(
            split_args(".import 'my file.csv' t"),
            vec![".import", "my file.csv", "t"]
        );
    }

    #[test]
    fn mode_parses_aliases() {
        assert_eq!(OutputMode::parse("box"), Some(OutputMode::Table));
        assert_eq!(OutputMode::parse("lines"), Some(OutputMode::Line));
        assert_eq!(OutputMode::parse("nope"), None);
    }
}
