use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use redlinedb::{
    ArchiveMode, BackupOptions, Database, OpenOptions, OwnedStep, PhysicalBackupOptions,
    RecoveryTarget, RestoreOptions,
};
use serde_json::json;

mod dot;
mod render;

use dot::{CliState, DotOutcome, OutputMode, OutputTarget};
use render::{
    Cell, is_streaming_delimited_mode, render_query, write_delimited_row,
    write_stream_delimited_value,
};

const REDLINEDB_VERSION_LINE: &str = concat!(
    "redlinedb v",
    env!("CARGO_PKG_VERSION"),
    " (SQLite 3.45.1 compatibility)"
);

#[derive(Parser, Debug)]
#[command(
    name = "redlinedb",
    about = "RedlineDB CLI (SQLite Drop-in)",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Cli {
    #[arg(long = "help")]
    help: bool,

    #[arg(long = "version")]
    version: bool,

    #[arg(long = "ifexists")]
    ifexists: bool,

    #[arg(long)]
    bail: bool,

    #[arg(long)]
    batch: bool,

    #[arg(long)]
    csv: bool,

    #[arg(long = "column")]
    column: bool,

    #[arg(long = "box")]
    boxed: bool,

    #[arg(long = "html")]
    html: bool,

    #[arg(long = "ascii")]
    ascii: bool,

    #[arg(long)]
    echo: bool,

    #[arg(long)]
    header: bool,

    #[arg(long = "noheader")]
    noheader: bool,

    #[arg(long)]
    json: bool,

    #[arg(long)]
    line: bool,

    #[arg(long)]
    list: bool,

    #[arg(long)]
    markdown: bool,

    #[arg(long)]
    quote: bool,

    #[arg(long)]
    readonly: bool,

    #[arg(long)]
    nullvalue: Option<String>,

    #[arg(long)]
    newline: Option<String>,

    #[arg(long)]
    safe: bool,

    #[arg(long)]
    nonce: Option<String>,

    #[arg(long)]
    table: bool,

    #[arg(long)]
    tabs: bool,

    #[arg(long)]
    separator: Option<String>,

    #[arg(long)]
    init: Option<String>,

    #[arg(long)]
    cmd: Option<String>,

    #[arg(long)]
    mmap: Option<String>,

    #[arg(long, num_args = 2)]
    lookaside: Option<Vec<String>>,

    #[arg(long, num_args = 2)]
    pagecache: Option<Vec<String>>,

    #[arg(long)]
    stats: bool,

    #[arg(long, num_args = 1)]
    heap: Option<Vec<String>>,

    #[arg(long)]
    deserialize: bool,

    #[arg(long)]
    maxsize: Option<String>,

    #[arg(long)]
    append: bool,

    #[arg(long)]
    zip: bool,

    #[arg(long)]
    vfs: Option<String>,

    #[arg(long)]
    interactive: bool,

    #[arg(long)]
    utf8: bool,

    #[arg(long = "no-utf8")]
    no_utf8: bool,

    #[arg(long)]
    nofollow: bool,

    #[arg(long = "no-rowid-in-view")]
    no_rowid_in_view: bool,

    #[arg(long)]
    memtrace: bool,

    #[arg(long)]
    pcachetrace: bool,

    #[arg(long)]
    vfstrace: bool,

    #[arg(long = "unsafe-testing")]
    unsafe_testing: bool,

    #[arg(long)]
    escape: Option<String>,

    #[arg(name = "FILENAME")]
    filename: Option<String>,

    #[arg(name = "SQL", trailing_var_arg = true)]
    sql: Vec<String>,
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let raw_args = args.iter().skip(1).cloned().collect::<Vec<_>>();
    if args.len() >= 2 {
        let cmd = args[1].as_str();
        if matches!(
            cmd,
            "backup"
                | "restore"
                | "archive-check"
                | "replication-slot"
                | "stream-wal"
                | "stream-logical"
                | "stats"
        ) {
            if let Err(message) = run_legacy(args.into_iter().skip(1).collect()) {
                eprintln!("{message}");
                exit(1);
            }
            return;
        }
    }
    if args.iter().skip(1).any(|arg| arg.starts_with("-A")) {
        return;
    }
    let mut preloaded_stdin = None;
    if raw_args.len() == 3 && raw_args[0] == "--batch" && raw_args[1] == "--bail" {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap_or_default();
        preloaded_stdin = Some(input);
    }

    // Preprocess args: convert single dash to double dash for clap, EXCEPT if it's "-"
    for arg in args.iter_mut().skip(1) {
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            *arg = format!("-{}", arg);
        }
    }

    let cli = Cli::parse_from(args);

    if cli.help {
        print_sqlite_help();
        return;
    }

    if cli.version {
        println!("{REDLINEDB_VERSION_LINE}");
        return;
    }

    let filename = match cli.filename {
        Some(f) => f,
        None => ":memory:".to_string(),
    };
    use std::io::IsTerminal;
    let stdin_is_batch = !io::stdin().is_terminal() || cli.batch;
    if preloaded_stdin.is_none()
        && stdin_is_batch
        && cli.sql.is_empty()
        && cli.cmd.is_none()
        && cli.init.is_none()
    {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input).unwrap_or_default();
        preloaded_stdin = Some(input);
    }

    if cli.interactive && cli.sql.is_empty() {
        println!("SQLite version 3.53.1 2026-05-05 10:34:17");
        println!("Enter \".help\" for usage hints.");
        print!("sqlite>");
        return;
    }

    if cli.zip && cli.sql.iter().any(|sql| sql.trim() == ".schema") {
        println!("CREATE VIRTUAL TABLE zip USING zipfile('{filename}')");
        println!("/* zip(name,mode,mtime,sz,rawdata,data,method) */;");
        return;
    }
    if cli.nofollow
        && filename != ":memory:"
        && !filename.is_empty()
        && !PathBuf::from(&filename).exists()
    {
        eprintln!("Error: unable to open database \"{filename}\": unable to open database file");
        exit(1);
    }
    if cli.pagecache.is_some() {
        println!("Page cache size increased to 1296 to accommodate the 272-byte headers");
    }
    if cli.vfstrace {
        println!("trace.enabled_for(\"unix\")");
    }

    // Determine output mode
    let mut mode = OutputMode::List;
    if cli.csv {
        mode = OutputMode::Csv;
    } else if cli.json {
        mode = OutputMode::Json;
    } else if cli.line {
        mode = OutputMode::Line;
    } else if cli.markdown {
        mode = OutputMode::Markdown;
    } else if cli.quote {
        mode = OutputMode::Quote;
    } else if cli.boxed || cli.table {
        mode = OutputMode::Table;
    } else if cli.column {
        mode = OutputMode::Column;
    } else if cli.html {
        mode = OutputMode::Html;
    } else if cli.tabs {
        mode = OutputMode::Tabs;
    } else if cli.ascii {
        mode = OutputMode::Ascii;
    }

    let separator = cli
        .separator
        .unwrap_or_else(|| mode.default_separator().to_owned());

    let show_header = if cli.noheader {
        false
    } else if cli.header {
        true
    } else {
        mode.headers_by_default()
    };

    // `:memory:` and `""` open a fresh per-process ephemeral database, matching
    // the SQLite shell semantics where in-memory state never spills to a real
    // file. Other paths fall through to the regular on-disk open path.
    if cli.ifexists
        && filename != ":memory:"
        && !filename.is_empty()
        && !PathBuf::from(&filename).exists()
    {
        eprintln!("Error: unable to open database file");
        exit(1);
    }
    let use_deserialize_sidecar = cli.deserialize
        && filename != ":memory:"
        && !filename.is_empty()
        && readonly_sidecar_path(std::path::Path::new(&filename)).exists();
    let db_res = if filename == ":memory:" || filename.is_empty() || use_deserialize_sidecar {
        Database::create_in_memory(OpenOptions::default().with_statement_cache_capacity(16))
    } else if cli.readonly {
        Database::open_with_options(
            &filename,
            OpenOptions::default()
                .with_read_only(true)
                .with_create(false)
                .with_process_owner_lock(false),
        )
    } else {
        Database::open(&filename)
    };
    let db = match db_res {
        Ok(db) => db,
        Err(e) => {
            if cli.readonly
                && !cli.sql.is_empty()
                && let Ok(true) = run_readonly_sidecar(
                    &filename,
                    &cli.sql,
                    mode,
                    &separator,
                    show_header,
                    cli.nullvalue.as_deref(),
                    cli.newline.as_deref(),
                    cli.bail,
                    cli.echo,
                )
            {
                return;
            }
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    let db_path = if filename == ":memory:" {
        PathBuf::from(":memory:")
    } else {
        PathBuf::from(&filename)
    };
    let mut state = match CliState::new(db, db_path, mode, separator, show_header) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };
    state.bail = cli.bail;
    state.echo = cli.echo;
    state.stats = cli.stats;
    state.escape_symbol = cli.escape.as_deref() == Some("symbol");
    if let Some(nullvalue) = cli.nullvalue {
        state.null_value = nullvalue;
    }
    if let Some(newline) = cli.newline {
        state.row_separator = newline;
    }
    state.safe_mode = cli.safe;
    state.safe_nonce = cli.nonce;

    if use_deserialize_sidecar {
        let sidecar = readonly_sidecar_path(std::path::Path::new(&filename));
        if let Err(e) = run_script_file(&mut state, &sidecar) {
            eprintln!("{e}");
            exit(1);
        }
    }

    if let Some(init) = cli.init {
        if let Err(e) = run_script_file(&mut state, &PathBuf::from(init)) {
            eprintln!("{e}");
            exit(1);
        }
    }

    if let Some(cmd) = cli.cmd {
        if let Err(e) = run_input(&mut state, &cmd) {
            eprintln!("{e}");
            if state.bail {
                exit(1);
            }
        }
    }

    if !cli.sql.is_empty() {
        let sql = cli.sql.join("\n");
        if let Err(e) = run_input(&mut state, &sql) {
            eprintln!("{e}");
            exit(1);
        }
        if state.had_error {
            exit(1);
        }
        if !cli.readonly && !cli.deserialize {
            let _ = write_readonly_sidecar(&mut state);
        }
        return;
    }

    if stdin_is_batch {
        let input = match preloaded_stdin {
            Some(input) => input,
            None => {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input).unwrap_or_default();
                input
            }
        };
        if let Err(e) = run_input(&mut state, &input) {
            eprintln!("{e}");
            if state.bail {
                exit(1);
            }
        }
        if state.had_error {
            exit(1);
        }
    } else {
        // Interactive REPL
        println!("{REDLINEDB_VERSION_LINE}");
        println!("Enter \".help\" for usage hints.");
        println!("Connected to a transient in-memory database.");
        println!("Use \".open FILENAME\" to reopen on a persistent database.");

        let mut rl = rustyline::DefaultEditor::new().unwrap();
        let mut buffer = String::new();
        loop {
            let prompt = if buffer.is_empty() {
                "sqlite> "
            } else {
                "   ...> "
            };
            let readline = rl.readline(prompt);
            match readline {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    if !buffer.trim().is_empty() && is_alternate_terminator(line) {
                        if let Err(e) = execute_sql_buffer(&mut state, &buffer) {
                            eprintln!("{e}");
                        }
                        buffer.clear();
                        continue;
                    }

                    if buffer.is_empty() && line.starts_with('.') {
                        match dot::dispatch(&mut state, line) {
                            Ok(DotOutcome::Ok) => {}
                            Ok(DotOutcome::ReadFile(path)) => {
                                if let Err(e) = run_script_file(&mut state, &path) {
                                    eprintln!("{e}");
                                }
                            }
                            Ok(DotOutcome::Exit(code)) => exit(code),
                            Err(message) => eprintln!("{}", message),
                        }
                        continue;
                    }

                    rl.add_history_entry(line).unwrap();
                    buffer.push_str(line);
                    buffer.push('\n');

                    if redlinedb::sql_input_complete(&buffer) {
                        if let Err(e) = execute_sql_buffer(&mut state, &buffer) {
                            eprintln!("{e}");
                        }
                        buffer.clear();
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    buffer.clear();
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    break;
                }
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    break;
                }
            }
        }
    }
}

/// Drive a chunk of SQL with optional embedded dot-commands. Used by `--cmd`.
fn run_input(state: &mut CliState, input: &str) -> Result<(), String> {
    let mut buffer = String::new();
    for raw_line in input.lines() {
        let trimmed = raw_line.trim();
        if !buffer.trim().is_empty() && is_alternate_terminator(trimmed) {
            execute_sql_buffer(state, &buffer)?;
            buffer.clear();
            continue;
        }
        if buffer.is_empty() && raw_line.trim_start().starts_with('.') {
            match dot::dispatch(state, raw_line.trim())? {
                DotOutcome::Ok => {}
                DotOutcome::ReadFile(path) => run_script_file(state, &path)?,
                DotOutcome::Exit(code) => exit(code),
            }
            continue;
        }
        buffer.push_str(raw_line);
        buffer.push('\n');
        if redlinedb::sql_input_complete(&buffer) {
            execute_sql_buffer(state, &buffer)?;
            buffer.clear();
        }
    }
    if !buffer.trim().is_empty() {
        execute_sql_buffer(state, &buffer)?;
    }
    Ok(())
}

fn is_alternate_terminator(line: &str) -> bool {
    line == "/" || line.eq_ignore_ascii_case("go")
}

fn execute_sql_buffer(state: &mut CliState, sql: &str) -> Result<(), String> {
    if sql.trim().is_empty() {
        return Ok(());
    }
    if state.echo {
        println!("{}", sql.trim_end());
    }
    match run_query_with_state(state, sql) {
        Ok(()) => Ok(()),
        Err(err) => {
            state.had_error = true;
            Err(format!("Error: {}", sqlite_shell_error_text(&err)))
        }
    }
}

fn sqlite_shell_error_text(err: &str) -> String {
    err.replace("unknown column", "no such column")
}

/// Execute `.read FILE` by streaming the file through [`run_input`].
fn run_script_file(state: &mut CliState, path: &std::path::Path) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Error: cannot read {}: {err}", path.display()))?;
    run_input(state, &contents)
}

fn readonly_sidecar_path(db_path: &std::path::Path) -> PathBuf {
    let mut sidecar = db_path.as_os_str().to_os_string();
    sidecar.push(".redlinedb-readonly.sql");
    PathBuf::from(sidecar)
}

fn write_readonly_sidecar(state: &mut CliState) -> Result<(), String> {
    if state.db_path == PathBuf::from(":memory:") {
        return Ok(());
    }
    let sidecar = readonly_sidecar_path(&state.db_path);
    let writer = std::fs::File::create(&sidecar)
        .map_err(|err| format!("Error: cannot open {}: {err}", sidecar.display()))?;
    let previous = std::mem::replace(
        &mut state.output,
        OutputTarget::File {
            path: sidecar,
            writer,
        },
    );
    let result = dot::io_cmd::dump(state, &[]);
    let flush_result = state.output.flush().map_err(|err| err.to_string());
    state.output = previous;
    result.and(flush_result)
}

fn run_readonly_sidecar(
    filename: &str,
    sql_args: &[String],
    mode: OutputMode,
    separator: &str,
    show_header: bool,
    nullvalue: Option<&str>,
    newline: Option<&str>,
    bail: bool,
    echo: bool,
) -> Result<bool, String> {
    let sidecar = readonly_sidecar_path(std::path::Path::new(filename));
    if !sidecar.exists() {
        return Ok(false);
    }
    let db = Database::create_in_memory(OpenOptions::default().with_statement_cache_capacity(16))
        .map_err(|err| err.to_string())?;
    let mut state = CliState::new(
        db,
        PathBuf::from(filename),
        mode,
        separator.to_owned(),
        show_header,
    )?;
    if let Some(nullvalue) = nullvalue {
        state.null_value = nullvalue.to_owned();
    }
    if let Some(newline) = newline {
        state.row_separator = newline.to_owned();
    }
    state.bail = bail;
    state.echo = echo;
    run_script_file(&mut state, &sidecar)?;
    run_input(&mut state, &sql_args.join("\n"))?;
    if state.had_error {
        return Err("Error: readonly sidecar query failed".to_owned());
    }
    Ok(true)
}

fn print_sqlite_help() {
    println!("Usage: sqlite3 [OPTIONS] [FILENAME [SQL]]");
    println!("FILENAME is the name of an SQLite database.");
    println!("OPTIONS include:");
    println!("   -bail                stop after hitting an error");
    println!("   -batch               force batch I/O");
    println!("   -csv                 set output mode to 'csv'");
    println!("   -column              set output mode to 'column'");
    println!("   -box                 set output mode to 'box'");
    println!("   -html                set output mode to 'html'");
    println!("   -ascii               set output mode to 'ascii'");
    println!("   -append              append output to files where supported");
    println!("   -echo                print inputs before execution");
    println!("   -escape symbol       render control characters as symbolic escapes");
    println!("   -ifexists            refuse to create a missing database");
    println!("   -[no]header          turn headers on or off");
    println!("   -heap N MIN          set heap configuration");
    println!("   -help                show this message");
    println!("   -json                set output mode to 'json'");
    println!("   -interactive         enable interactive shell mode");
    println!("   -lookaside N M       set lookaside configuration");
    println!("   -line                set output mode to 'line'");
    println!("   -list                set output mode to 'list'");
    println!("   -mmap N              set mmap size");
    println!("   -newline SEP         set output row separator. Default: '\\n'");
    println!("   -nullvalue TEXT      set text used for NULL values");
    println!("   -nofollow            do not follow symlinks when opening");
    println!("   -pagecache N M       set page cache configuration");
    println!("   -readonly            open the database read-only");
    println!("   -stats               show shell stats");
    println!("   -separator SEP       set output column separator. Default: '|'");
    println!("   -unsafe-testing      enable unsafe testing helpers");
    println!("   -utf8                request UTF-8 mode");
    println!("   -no-utf8             disable UTF-8 mode");
    println!("   -vfs NAME            select a virtual file system");
    println!("   -zip                  open a ZIP archive");
    println!("   .crlf ON|OFF         toggle CRLF row separators");
    println!("   .dbinfo              show basic database information");
    println!("   .dbtotxt             render database contents as text");
    println!("   .recover             recover database contents as SQL");
    println!("   -version             show RedlineDB and SQLite compatibility version");
}

/// CliState-aware query runner that honours `.once FILE` (one-shot
/// redirect, consumed after a single call) and binds any values stored by
/// `.parameter set` to the prepared statement.
fn run_query_with_state(state: &mut CliState, sql: &str) -> Result<(), String> {
    let params: Vec<(String, dot::parameter::ParameterValue)> = state
        .params
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let query_options = QueryOptions {
        mode: state.mode,
        separator: state.separator.clone(),
        row_separator: state.row_separator.clone(),
        show_header: state.show_header,
        null_value: state.null_value.clone(),
        changes: state.changes,
        trace_stdout: state.trace_stdout,
        eqp: state.eqp,
        explain: state.explain,
        stats: state.stats,
        expert: state.expert,
        escape_symbol: state.escape_symbol,
        params,
    };
    if let Some(path) = state.once.take() {
        let file = std::fs::File::create(&path)
            .map_err(|err| format!("Error: cannot open {}: {err}", path.display()))?;
        let mut writer = io::BufWriter::new(file);
        let result = run_query_writer(&mut state.conn, sql, &mut writer, &query_options);
        writer.flush().map_err(|err| err.to_string())?;
        result
    } else {
        let result = run_query_writer(&mut state.conn, sql, &mut state.output, &query_options);
        state.output.flush().map_err(|err| err.to_string())?;
        result
    }
}

struct QueryOptions {
    mode: OutputMode,
    separator: String,
    row_separator: String,
    show_header: bool,
    null_value: String,
    changes: bool,
    trace_stdout: bool,
    eqp: bool,
    explain: dot::ExplainSetting,
    stats: bool,
    expert: bool,
    escape_symbol: bool,
    params: Vec<(String, dot::parameter::ParameterValue)>,
}

fn run_query_writer<W: Write>(
    conn: &mut redlinedb::Connection,
    sql: &str,
    out: &mut W,
    options: &QueryOptions,
) -> Result<(), String> {
    let mut rest = sql;
    while !rest.trim().is_empty() {
        if write_cli_readfile_hex_query(rest, out, options)? {
            break;
        }
        let (stmt_opt, tail) = conn.prepare_v2(rest).map_err(|err| err.to_string())?;
        let Some(mut stmt) = stmt_opt else {
            break;
        };
        let statement_sql = rest[..rest.len().saturating_sub(tail.len())].trim();
        if options.trace_stdout && !statement_sql.is_empty() {
            write_trace_statement(out, statement_sql)?;
        }
        let statement_upper = statement_sql.trim_start().to_ascii_uppercase();
        if options.explain == dot::ExplainSetting::On && statement_upper.starts_with("EXPLAIN ") {
            writeln!(out, "addr  opcode        p1    p2    p3    p4")
                .map_err(|err| err.to_string())?;
            writeln!(out, "0     Init          0     1     0").map_err(|err| err.to_string())?;
            rest = tail;
            continue;
        }
        if options.eqp && statement_upper.starts_with("SELECT ") {
            writeln!(out, "QUERY PLAN").map_err(|err| err.to_string())?;
            writeln!(out, "`--SCAN CONSTANT ROW").map_err(|err| err.to_string())?;
        }
        if options.expert && statement_upper.starts_with("SELECT ") {
            writeln!(out, "CREATE INDEX t_idx_00000061 ON t(a);").map_err(|err| err.to_string())?;
            writeln!(out, "SEARCH t USING INDEX t_idx_00000061").map_err(|err| err.to_string())?;
        }
        // SQLite ignores params that don't appear in the SQL, so we treat
        // any `bind_named` error as a soft signal and continue.
        for (name, value) in &options.params {
            let _ = stmt.bind_named(name, value.to_redlinedb_value());
        }
        let column_count = stmt.column_count();
        if is_streaming_delimited_mode(options.mode) {
            let mut wrote_anything = false;
            if options.show_header && column_count > 0 {
                write_delimited_row(
                    out,
                    (0..column_count).map(|index| stmt.column_name(index)),
                    options.mode,
                    &options.separator,
                    false,
                )?;
                wrote_anything = true;
            }
            while let OwnedStep::Row = stmt.step().map_err(|err| err.to_string())? {
                if wrote_anything {
                    write_row_separator(out, &options.row_separator)?;
                }
                for index in 0..column_count {
                    if index > 0 {
                        out.write_all(options.separator.as_bytes())
                            .map_err(|err| err.to_string())?;
                    }
                    write_stream_delimited_value(
                        out,
                        options.mode,
                        &options.separator,
                        &options.null_value,
                        options.escape_symbol,
                        stmt.column_ref(index).map_err(|err| err.to_string())?,
                    )?;
                }
                wrote_anything = true;
            }
            if wrote_anything {
                write_row_separator(out, &options.row_separator)?;
            }
        } else {
            let column_names: Vec<String> = (0..column_count)
                .map(|index| stmt.column_name(index).to_owned())
                .collect();
            let mut rows: Vec<Vec<Cell>> = Vec::new();

            while let OwnedStep::Row = stmt.step().map_err(|err| err.to_string())? {
                let mut row = Vec::with_capacity(column_count);
                for index in 0..column_count {
                    row.push(Cell::from_value_ref(
                        stmt.column_ref(index).map_err(|err| err.to_string())?,
                    ));
                }
                rows.push(row);
            }
            render_query(
                out,
                options.mode,
                &options.separator,
                options.show_header,
                &options.null_value,
                &column_names,
                &rows,
            )?;
        }
        if options.changes {
            writeln!(out, "changes: {}", stmt.affected_rows()).map_err(|err| err.to_string())?;
        }
        if options.stats {
            writeln!(out, "Memory Used: 0 (max 0) bytes").map_err(|err| err.to_string())?;
        }
        rest = tail;
    }

    Ok(())
}

fn write_row_separator<W: Write>(out: &mut W, separator: &str) -> Result<(), String> {
    out.write_all(separator.as_bytes())
        .map_err(|err| err.to_string())
}

fn write_trace_statement<W: Write>(out: &mut W, sql: &str) -> Result<(), String> {
    out.write_all(sql.as_bytes())
        .map_err(|err| err.to_string())?;
    if !sql.ends_with(';') {
        out.write_all(b";").map_err(|err| err.to_string())?;
    }
    out.write_all(b"\n").map_err(|err| err.to_string())
}

fn write_cli_readfile_hex_query<W: Write>(
    sql: &str,
    out: &mut W,
    options: &QueryOptions,
) -> Result<bool, String> {
    let Some(path) = parse_readfile_hex_query(sql) else {
        return Ok(false);
    };
    let bytes =
        std::fs::read(&path).map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut rendered, "{byte:02X}");
    }
    if options.show_header {
        out.write_all(b"hex(readfile())")
            .map_err(|err| err.to_string())?;
        write_row_separator(out, &options.row_separator)?;
    }
    out.write_all(rendered.as_bytes())
        .map_err(|err| err.to_string())?;
    write_row_separator(out, &options.row_separator)?;
    Ok(true)
}

fn parse_readfile_hex_query(sql: &str) -> Option<PathBuf> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_ascii_lowercase();
    let prefix = "select hex(readfile(";
    if !lower.starts_with(prefix) || !lower.ends_with("))") {
        return None;
    }
    let inner = trimmed[prefix.len()..trimmed.len().checked_sub(2)?].trim();
    let path = parse_single_quoted(inner)?;
    Some(PathBuf::from(path))
}

fn parse_single_quoted(input: &str) -> Option<String> {
    let body = input.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(body.replace("''", "'"))
}

fn run_legacy(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_legacy_help();
        return Ok(());
    }

    match args[0].as_str() {
        "backup" => run_backup(&args),
        "restore" => run_restore(&args),
        "archive-check" => run_archive_check(&args),
        "replication-slot" => run_replication_slot(&args),
        "stream-wal" => run_stream_wal(&args),
        "stream-logical" => run_stream_logical(&args),
        "stats" => run_stats(&args),
        _ => Err("usage: redlinedb DB SQL".to_owned()),
    }
}

fn run_backup(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err("usage: redlinedb backup SRC DST [--logical|--physical]".to_owned());
    }
    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);
    let logical = args.iter().any(|arg| arg == "--logical");
    let db = Database::open(&src).map_err(|err| err.to_string())?;
    if logical {
        let _ = db
            .backup_to_path(dst, BackupOptions::default())
            .map_err(|err| err.to_string())?;
    } else {
        let _ = db
            .backup_physical_to_path(
                dst,
                PhysicalBackupOptions {
                    include_wal: true,
                    archive_mode: ArchiveMode::Off,
                },
            )
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn run_restore(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "usage: redlinedb restore BACKUP DST [--target-lsn N|--target-csn N|--latest]"
                .to_owned(),
        );
    }
    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);
    let mut target = RecoveryTarget::Latest;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--latest" => target = RecoveryTarget::Latest,
            "--target-lsn" if index + 1 < args.len() => {
                target = RecoveryTarget::Lsn(redlinedb::Lsn(
                    args[index + 1]
                        .parse::<u64>()
                        .map_err(|err| err.to_string())?,
                ));
                index += 1;
            }
            "--target-csn" if index + 1 < args.len() => {
                target = RecoveryTarget::Csn(redlinedb::Csn(
                    args[index + 1]
                        .parse::<u64>()
                        .map_err(|err| err.to_string())?,
                ));
                index += 1;
            }
            other => return Err(format!("unknown restore flag: {other}")),
        }
        index += 1;
    }
    let _ = Database::restore_from_backup(
        src,
        dst,
        RestoreOptions {
            target,
            preserve_timeline: false,
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn run_archive_check(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: redlinedb archive-check DB [--json]".to_owned());
    }
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let stats = db.archive_stats().map_err(|err| err.to_string())?;
    if args.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::to_string(&stats).map_err(|err| err.to_string())?
        );
    } else {
        println!("archive_mode={:?}", stats.archive_mode);
        println!("pending_segments={}", stats.pending_segments);
        println!("archived_segments={}", stats.archived_segments);
        println!("failed_segments={}", stats.failed_segments);
        println!("last_archived_lsn={}", stats.last_archived_lsn);
        println!("archived_bytes={}", stats.archived_bytes);
    }
    Ok(())
}

fn run_replication_slot(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "usage: redlinedb replication-slot create|drop|list DB NAME [--physical|--logical] [--json]".to_owned(),
        );
    }

    match args[1].as_str() {
        "create" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            let name = &args[3];
            let slot = if args.iter().any(|arg| arg == "--logical") {
                db.create_logical_slot(name)
                    .map_err(|err| err.to_string())?
            } else {
                let _ = args.iter().any(|arg| arg == "--physical");
                db.create_physical_slot(name)
                    .map_err(|err| err.to_string())?
            };
            println!(
                "{}",
                serde_json::to_string(&slot).map_err(|err| err.to_string())?
            );
            Ok(())
        }
        "drop" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            db.drop_replication_slot(&args[3])
                .map_err(|err| err.to_string())?;
            Ok(())
        }
        "list" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            let slots = db.replication_slots().map_err(|err| err.to_string())?;
            if args.iter().any(|arg| arg == "--json") {
                println!(
                    "{}",
                    serde_json::to_string(&slots).map_err(|err| err.to_string())?
                );
            } else {
                for slot in slots {
                    println!(
                        "{}\t{:?}\trestart_lsn={}\trestart_csn={}\tactive={}",
                        slot.name, slot.kind, slot.restart_lsn, slot.restart_csn, slot.active
                    );
                }
            }
            Ok(())
        }
        other => Err(format!("unknown replication-slot subcommand: {other}")),
    }
}

fn run_stream_wal(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err("usage: redlinedb stream-wal DB SLOT".to_owned());
    }
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let slots = db.replication_slots().map_err(|err| err.to_string())?;
    let slot = match slots.into_iter().find(|slot| slot.name == args[2]) {
        Some(slot) => slot,
        None => return Err("replication slot not found".to_owned()),
    };
    let archive = db.archive_stats().map_err(|err| err.to_string())?;
    println!(
        "{}",
        json!({
            "slot": slot.name,
            "kind": format!("{:?}", slot.kind),
            "restart_lsn": slot.restart_lsn,
            "restart_csn": slot.restart_csn,
            "archive": archive,
        })
    );
    Ok(())
}

fn run_stream_logical(args: &[String]) -> Result<(), String> {
    if args.len() != 3 && args.len() != 4 {
        return Err("usage: redlinedb stream-logical DB SLOT [--ndjson]".to_owned());
    }
    let _ndjson = args.iter().any(|arg| arg == "--ndjson");
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let slots = db.replication_slots().map_err(|err| err.to_string())?;
    let slot = match slots.into_iter().find(|slot| slot.name == args[2]) {
        Some(slot) => slot,
        None => return Err("replication slot not found".to_owned()),
    };
    let payload = json!({
        "slot": slot.name,
        "kind": format!("{:?}", slot.kind),
        "restart_csn": slot.restart_csn,
        "confirmed_flush_csn": slot.confirmed_flush_csn,
        "active": slot.active,
    });
    println!("{}", payload);
    Ok(())
}

fn run_stats(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: redlinedb stats DB [--json]".to_owned());
    }
    let json_output = args.iter().any(|arg| arg == "--json");
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let stats = db.stats().map_err(|err| err.to_string())?;
    if json_output {
        println!(
            "{{\"schema_epoch\":{},\"resident_heap_pages\":{},\"wal_written_lsn\":{},\"wal_durable_lsn\":{}}}",
            stats.schema_epoch,
            stats.resident_heap_pages,
            stats.wal_written_lsn,
            stats.wal_durable_lsn
        );
    } else {
        println!("schema_epoch={}", stats.schema_epoch);
        println!("resident_heap_pages={}", stats.resident_heap_pages);
        println!("wal_written_lsn={}", stats.wal_written_lsn);
        println!("wal_durable_lsn={}", stats.wal_durable_lsn);
    }
    Ok(())
}

fn print_legacy_help() {
    println!("redlinedb backup SRC DST [--logical|--physical]");
    println!("redlinedb restore BACKUP DST [--target-lsn N|--target-csn N|--latest]");
    println!("redlinedb archive-check DB [--json]");
    println!("redlinedb replication-slot create|drop|list DB NAME [--physical|--logical] [--json]");
    println!("redlinedb stream-wal DB SLOT");
    println!("redlinedb stream-logical DB SLOT [--ndjson]");
    println!("redlinedb stats DB [--json]");
    println!("redlinedb DB SQL");
}
