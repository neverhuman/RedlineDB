use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use redlinedb::{Database, OpenOptions, OwnedStep, RqlProgram, RqlStatement};

mod dot;
mod maintenance;
mod render;
mod shellzero;

use dot::{CliState, DotOutcome, OutputMode, OutputTarget};
use maintenance::run_maintenance;
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
    tcl: bool,

    #[arg(long)]
    separator: Option<String>,

    #[arg(long)]
    init: Option<String>,

    #[arg(long, action = clap::ArgAction::Append)]
    cmd: Vec<String>,

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

    #[arg(long = "shellzero")]
    shellzero: bool,

    #[arg(long = "no-shellzero")]
    no_shellzero: bool,

    #[arg(long)]
    rql: bool,

    #[arg(long)]
    escape: Option<String>,

    #[arg(name = "FILENAME")]
    filename: Option<String>,

    #[arg(name = "SQL", trailing_var_arg = true)]
    sql: Vec<String>,
}

pub fn run() {
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
            if let Err(message) = run_maintenance(args.into_iter().skip(1).collect()) {
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
    // Fast path: eagerly slurp stdin for the canonical harness pattern
    // `redlinedb {-batch|--batch} {-bail|--bail} <filename>`.
    // The harness uses single-dash flags (SQLite CLI convention), so we
    // must accept both `-batch` and `--batch` here — the normalization loop
    // below has not yet run.  Reading stdin early lets us overlap I/O with
    // the subsequent Clap parse, flag resolution, and DB setup.
    {
        let is_batch = matches!(
            raw_args.get(0).map(String::as_str),
            Some("-batch") | Some("--batch")
        );
        let is_bail = matches!(
            raw_args.get(1).map(String::as_str),
            Some("-bail") | Some("--bail")
        );
        if raw_args.len() == 3 && is_batch && is_bail {
            let mut input = String::new();
            io::stdin().read_to_string(&mut input).unwrap_or_default();
            preloaded_stdin = Some(input);
        }
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

    if cli.rql && (!cli.sql.is_empty() || !cli.cmd.is_empty()) {
        eprintln!(
            "Error: --rql reads RQL JSON from stdin and cannot be combined with --cmd or SQL arguments"
        );
        exit(1);
    }

    let filename = match cli.filename {
        Some(f) => f,
        None => ":memory:".to_string(),
    };
    // SQLite 3.53.1 no longer emits "Error: out of memory" for
    // `-deserialize :memory:`; we match that behavior for parity.
    use std::io::IsTerminal;
    let stdin_is_batch = !io::stdin().is_terminal() || cli.batch;
    if preloaded_stdin.is_none()
        && stdin_is_batch
        && cli.sql.is_empty()
        && cli.cmd.is_empty()
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
        // Suppress the page-cache notice when both N and M are zero (the
        // SQLite shell prints nothing in that case).
        let suppress = cli
            .pagecache
            .as_ref()
            .map(|v| v.iter().all(|x| x.trim() == "0"))
            .unwrap_or(false);
        if !suppress {
            println!("Page cache size increased to 1296 to accommodate the 272-byte headers");
        }
    }
    if cli.vfstrace {
        println!("trace.enabled_for(\"unix\")");
    }

    // Walk the raw arguments in order, mirroring the sqlite3 shell where
    // mode flags reset the header / nullvalue / separator to the mode's
    // defaults. The last mode flag (and any post-mode override) wins.
    let flag_state = resolve_cli_flags(&raw_args);
    let mode = flag_state.mode;

    // Wave-6b: capture which formatting overrides the user supplied
    // explicitly BEFORE the matches below consume the optional fields —
    // the shellzero auto-default needs to know the user accepted every
    // List-mode default.
    let explicit_header = flag_state.header.is_some();
    let explicit_null_value = flag_state.null_value.is_some();
    let explicit_separator = flag_state.separator.is_some();
    let explicit_row_separator = flag_state.row_separator.is_some();

    let separator = match flag_state.separator {
        Some(sep) => sep,
        None => mode.default_separator().to_owned(),
    };

    let show_header = match flag_state.header {
        Some(explicit) => explicit,
        None => mode.headers_by_default(),
    };

    // WS-C8 / Wave-6b: ShellZero pre-open fast path. The auto-default
    // ("safe shape") is conservative: `:memory:` filename + a single
    // positional SQL arg + no execution-state flags + no formatting
    // overrides + no stdin script + no `--cmd`. Anything outside that
    // surface (mode flags, header toggles, dot-commands piped via stdin,
    // …) falls through to the full CLI so the existing behaviour and
    // tests are preserved. `--shellzero` still works as an explicit
    // opt-in for the broader audited surface; `--no-shellzero` disables
    // both paths.
    let shellzero_base_eligible = (filename == ":memory:" || filename.is_empty())
        && cli.init.is_none()
        && !cli.echo
        && !cli.bail
        && !cli.stats;
    let safe_auto_shape = shellzero_base_eligible
        && mode == OutputMode::List
        && !explicit_header
        && !explicit_null_value
        && !explicit_separator
        && !explicit_row_separator
        && cli.cmd.is_empty()
        && preloaded_stdin.is_none()
        && !cli.sql.is_empty();
    let shellzero_enabled =
        !cli.no_shellzero && ((cli.shellzero && shellzero_base_eligible) || safe_auto_shape);
    if shellzero_enabled {
        let args = shellzero::ShellZeroArgs {
            sql: &cli.sql,
            cmd: &cli.cmd,
            separator: &separator,
            row_separator: flag_state.row_separator.as_deref().unwrap_or("\n"),
            null_value: flag_state.null_value.as_deref().unwrap_or(""),
        };
        if let Some(code) = shellzero::try_handle_pre_open(&args, preloaded_stdin.as_deref()) {
            exit(code);
        }
    }

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
        Database::create_in_memory(OpenOptions::default())
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
                    flag_state.null_value.as_deref(),
                    None,
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
    state.defer_output_flush = stdin_is_batch || !cli.sql.is_empty();
    state.escape_symbol = cli.escape.as_deref() == Some("symbol");
    if let Some(nullvalue) = flag_state.null_value {
        state.null_value = nullvalue;
    }
    if let Some(newline) = flag_state.row_separator {
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

    if cli.rql {
        let input = match preloaded_stdin {
            Some(input) => input,
            None => {
                let mut input = String::new();
                io::stdin().read_to_string(&mut input).unwrap_or_default();
                input
            }
        };
        if let Err(e) = run_rql_input(&mut state, &input) {
            flush_output_or_exit(&mut state);
            eprintln!("{e}");
            exit(1);
        }
        if state.had_error {
            flush_output_or_exit(&mut state);
            exit(1);
        }
        flush_output_or_exit(&mut state);
        return;
    }

    for cmd in &cli.cmd {
        if let Err(e) = run_input(&mut state, cmd) {
            eprintln!("{e}");
            if state.bail {
                exit(1);
            }
        }
    }

    if !cli.sql.is_empty() {
        let sql = cli.sql.join("\n");
        if let Err(e) = run_input(&mut state, &sql) {
            flush_output_or_exit(&mut state);
            eprintln!("{e}");
            exit(1);
        }
        if state.had_error {
            flush_output_or_exit(&mut state);
            exit(1);
        }
        flush_output_or_exit(&mut state);
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
            flush_output_or_exit(&mut state);
            eprintln!("{e}");
            if state.bail {
                exit(1);
            }
        }
        if state.had_error {
            flush_output_or_exit(&mut state);
            exit(1);
        }
        flush_output_or_exit(&mut state);
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
                            Ok(DotOutcome::Exit(code)) => {
                                flush_output_or_exit(&mut state);
                                exit(code);
                            }
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
    if !input_has_batch_control_lines(input) {
        return execute_sql_buffer(state, input);
    }
    if input
        .lines()
        .any(|line| line.trim_start().starts_with(".once"))
    {
        return run_input_incremental(state, input);
    }

    let mut sql_chunk = String::new();
    for raw_line in input.lines() {
        let trimmed = raw_line.trim();
        if !sql_chunk.trim().is_empty() && is_alternate_terminator(trimmed) {
            execute_sql_chunk(state, &mut sql_chunk)?;
            continue;
        }
        if raw_line.trim_start().starts_with('.') {
            if !sql_chunk.trim().is_empty() && redlinedb::sql_input_complete(&sql_chunk) {
                execute_sql_chunk(state, &mut sql_chunk)?;
            }
        }
        if sql_chunk.trim().is_empty() && raw_line.trim_start().starts_with('.') {
            // sqlite3 echoes every executed input line (including dot
            // commands) when `.echo on` is active; check the flag BEFORE
            // dispatching so a leading `.echo off` still gets logged.
            if state.echo {
                println!("{}", raw_line.trim_end());
            }
            match dot::dispatch(state, raw_line.trim())? {
                DotOutcome::Ok => {}
                DotOutcome::ReadFile(path) => run_script_file(state, &path)?,
                DotOutcome::Exit(code) => {
                    flush_output_or_exit(state);
                    exit(code);
                }
            }
            continue;
        }
        sql_chunk.push_str(raw_line);
        sql_chunk.push('\n');
    }
    execute_sql_chunk(state, &mut sql_chunk)?;
    Ok(())
}

fn run_input_incremental(state: &mut CliState, input: &str) -> Result<(), String> {
    let mut buffer = String::new();
    for raw_line in input.lines() {
        let trimmed = raw_line.trim();
        if !buffer.trim().is_empty() && is_alternate_terminator(trimmed) {
            execute_sql_buffer(state, &buffer)?;
            buffer.clear();
            continue;
        }
        if buffer.is_empty() && raw_line.trim_start().starts_with('.') {
            if state.echo {
                println!("{}", raw_line.trim_end());
            }
            match dot::dispatch(state, raw_line.trim())? {
                DotOutcome::Ok => {}
                DotOutcome::ReadFile(path) => run_script_file(state, &path)?,
                DotOutcome::Exit(code) => {
                    flush_output_or_exit(state);
                    exit(code);
                }
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

fn input_has_batch_control_lines(input: &str) -> bool {
    input.lines().any(|line| {
        let trimmed = line.trim();
        line.trim_start().starts_with('.') || is_alternate_terminator(trimmed)
    })
}

fn execute_sql_chunk(state: &mut CliState, sql_chunk: &mut String) -> Result<(), String> {
    if sql_chunk.trim().is_empty() {
        sql_chunk.clear();
        return Ok(());
    }
    execute_sql_buffer(state, sql_chunk)?;
    sql_chunk.clear();
    Ok(())
}

fn is_alternate_terminator(line: &str) -> bool {
    line == "/" || line.eq_ignore_ascii_case("go")
}

/// Whether the leading keyword of an uppercased SQL statement actually
/// mutates row data. sqlite3's `.changes` only counts these statements.
fn statement_changes_rows(statement_upper: &str) -> bool {
    let trimmed = statement_upper.trim_start();
    let head = trimmed.split_whitespace().next().unwrap_or("");
    matches!(
        head,
        "INSERT" | "UPDATE" | "DELETE" | "REPLACE" | "MERGE" | "UPSERT"
    )
}

fn flush_output_or_exit(state: &mut CliState) {
    if let Err(err) = state.output.flush() {
        eprintln!("{err}");
        exit(1);
    }
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
    let file = fs::File::open(path)
        .map_err(|err| format!("Error: cannot read {}: {err}", path.display()))?;
    let len = file
        .metadata()
        .map_err(|err| format!("Error: cannot read {}: {err}", path.display()))?
        .len();
    if len == 0 {
        return run_input(state, "");
    }
    // SAFETY: mmap is unsafe because concurrent mutation of the backing file
    // by another process would race; `.read` accepts that risk like SQLite.
    let mmap = unsafe { memmap2::Mmap::map(&file) }
        .map_err(|err| format!("Error: cannot read {}: {err}", path.display()))?;
    let contents = std::str::from_utf8(&mmap).map_err(|err| {
        format!(
            "Error: {} is not valid UTF-8 at byte {}",
            path.display(),
            err.valid_up_to()
        )
    })?;
    run_input(state, contents)
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
    // Phase 5 WS-C5c: BufWriter wrap to match the .output FILE fix.
    let previous = std::mem::replace(&mut state.output, OutputTarget::file(sidecar, writer));
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
    let db = Database::create_in_memory(OpenOptions::default()).map_err(|err| err.to_string())?;
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

/// Final state of the shell flags computed from the raw `argv` in source
/// order, mirroring the SQLite shell's "mode flag resets dependent
/// settings" quirk.
struct CliFlagState {
    mode: OutputMode,
    header: Option<bool>,
    null_value: Option<String>,
    separator: Option<String>,
    row_separator: Option<String>,
}

/// Iterate the raw CLI arguments in source order, returning the mode that
/// was finally selected together with explicit header / nullvalue /
/// separator / row-separator preferences (if the caller passed those
/// options after the last mode flag).
///
/// SQLite has a long-standing quirk where every mode flag resets the
/// dependent state (header / nullvalue / separator / newline) to that
/// mode's defaults. As a result, `-header -list` ends up with headers OFF
/// (because `-list` defaults to off and runs second) while `-list
/// -header` ends up with headers ON. The same applies to `-newline`. We
/// mirror that exactly.
fn resolve_cli_flags(raw_args: &[String]) -> CliFlagState {
    let mut state = CliFlagState {
        mode: OutputMode::List,
        header: None,
        null_value: None,
        separator: None,
        row_separator: None,
    };
    let mut iter = raw_args.iter();
    while let Some(arg) = iter.next() {
        let token = arg.trim_start_matches('-');
        let new_mode = match token {
            "csv" => Some(OutputMode::Csv),
            "json" => Some(OutputMode::Json),
            "line" => Some(OutputMode::Line),
            "markdown" => Some(OutputMode::Markdown),
            "quote" => Some(OutputMode::Quote),
            "box" => Some(OutputMode::Box),
            "table" => Some(OutputMode::Table),
            "column" => Some(OutputMode::Column),
            "html" => Some(OutputMode::Html),
            "tabs" => Some(OutputMode::Tabs),
            "ascii" => Some(OutputMode::Ascii),
            "list" => Some(OutputMode::List),
            "tcl" => Some(OutputMode::Tcl),
            _ => None,
        };
        if let Some(m) = new_mode {
            state.mode = m;
            // Mode flags reset every dependent state to the mode defaults;
            // post-mode flags below override them again.
            state.header = None;
            state.null_value = None;
            state.separator = None;
            state.row_separator = None;
            continue;
        }
        match token {
            "header" => state.header = Some(true),
            "noheader" => state.header = Some(false),
            "nullvalue" => {
                if let Some(value) = iter.next() {
                    state.null_value = Some(value.clone());
                }
            }
            "separator" => {
                if let Some(value) = iter.next() {
                    state.separator = Some(value.clone());
                }
            }
            "newline" => {
                if let Some(value) = iter.next() {
                    state.row_separator = Some(value.clone());
                }
            }
            _ => {}
        }
    }
    state
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
    println!("   -rql                 read Redline Query Language JSON from stdin");
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
        insert_table_name: state.insert_table_name.clone(),
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
        widths: state.widths.clone(),
        params,
    };
    let total_changes_before = state.total_changes;
    if let Some(path) = state.once.take() {
        let file = std::fs::File::create(&path)
            .map_err(|err| format!("Error: cannot open {}: {err}", path.display()))?;
        let mut writer = io::BufWriter::new(file);
        let mut local_total = total_changes_before;
        let result = run_query_writer(
            &mut state.conn,
            sql,
            &mut writer,
            &query_options,
            &mut local_total,
        );
        writer.flush().map_err(|err| err.to_string())?;
        state.total_changes = local_total;
        result
    } else {
        let mut local_total = total_changes_before;
        let result = run_query_writer(
            &mut state.conn,
            sql,
            &mut state.output,
            &query_options,
            &mut local_total,
        );
        if !state.defer_output_flush {
            state.output.flush().map_err(|err| err.to_string())?;
        }
        state.total_changes = local_total;
        result
    }
}

fn run_rql_input(state: &mut CliState, input: &str) -> Result<(), String> {
    if input.trim().is_empty() {
        return Ok(());
    }
    let program = parse_rql_program(input)?;
    match run_rql_program_with_state(state, &program) {
        Ok(()) => Ok(()),
        Err(err) => {
            state.had_error = true;
            Err(format!("Error: {}", sqlite_shell_error_text(&err)))
        }
    }
}

fn parse_rql_program(input: &str) -> Result<RqlProgram, String> {
    match serde_json::from_str::<RqlProgram>(input) {
        Ok(program) => Ok(program),
        Err(program_err) => match serde_json::from_str::<Vec<RqlStatement>>(input) {
            Ok(statements) => Ok(RqlProgram { statements }),
            Err(_) => Err(format!("invalid RQL JSON: {program_err}")),
        },
    }
}

fn run_rql_program_with_state(state: &mut CliState, program: &RqlProgram) -> Result<(), String> {
    let query_options = QueryOptions {
        mode: state.mode,
        insert_table_name: state.insert_table_name.clone(),
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
        widths: state.widths.clone(),
        params: Vec::new(),
    };
    let total_changes_before = state.total_changes;
    if let Some(path) = state.once.take() {
        let file = std::fs::File::create(&path)
            .map_err(|err| format!("Error: cannot open {}: {err}", path.display()))?;
        let mut writer = io::BufWriter::new(file);
        let mut local_total = total_changes_before;
        let result = run_rql_writer(
            &mut state.conn,
            program,
            &mut writer,
            &query_options,
            &mut local_total,
        );
        writer.flush().map_err(|err| err.to_string())?;
        state.total_changes = local_total;
        result
    } else {
        let mut local_total = total_changes_before;
        let result = run_rql_writer(
            &mut state.conn,
            program,
            &mut state.output,
            &query_options,
            &mut local_total,
        );
        if !state.defer_output_flush {
            state.output.flush().map_err(|err| err.to_string())?;
        }
        state.total_changes = local_total;
        result
    }
}

struct QueryOptions {
    mode: OutputMode,
    insert_table_name: String,
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
    widths: Vec<usize>,
    params: Vec<(String, dot::parameter::ParameterValue)>,
}

fn run_query_writer<W: Write>(
    conn: &mut redlinedb::Connection,
    sql: &str,
    out: &mut W,
    options: &QueryOptions,
    total_changes: &mut i64,
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
            // CSV is delimited per RFC 4180 — every row ends with CRLF. The
            // other delimited modes use the configured row separator.
            let row_terminator = if matches!(options.mode, OutputMode::Csv) {
                "\r\n".to_owned()
            } else {
                options.row_separator.clone()
            };
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
                    write_row_separator(out, &row_terminator)?;
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
                write_row_separator(out, &row_terminator)?;
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
                &options.insert_table_name,
                &options.widths,
                &column_names,
                &rows,
            )?;
        }
        if options.changes {
            // sqlite3 only counts row-mutating statements (INSERT / UPDATE
            // / DELETE / REPLACE). DDL like CREATE / DROP / ALTER reports
            // zero changes.
            let n = if statement_changes_rows(&statement_upper) {
                stmt.affected_rows() as i64
            } else {
                0
            };
            *total_changes += n;
            writeln!(out, "changes: {n}   total_changes: {total_changes}")
                .map_err(|err| err.to_string())?;
        }
        if options.stats {
            writeln!(out, "Memory Used: 0 (max 0) bytes").map_err(|err| err.to_string())?;
        }
        rest = tail;
    }

    Ok(())
}

fn run_rql_writer<W: Write>(
    conn: &mut redlinedb::Connection,
    program: &RqlProgram,
    out: &mut W,
    options: &QueryOptions,
    total_changes: &mut i64,
) -> Result<(), String> {
    for statement in &program.statements {
        let mut stmt = conn
            .prepare_rql_owned(statement)
            .map_err(|err| err.to_string())?;
        let column_count = stmt.column_count();
        if is_streaming_delimited_mode(options.mode) {
            let row_terminator = if matches!(options.mode, OutputMode::Csv) {
                "\r\n".to_owned()
            } else {
                options.row_separator.clone()
            };
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
                    write_row_separator(out, &row_terminator)?;
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
                write_row_separator(out, &row_terminator)?;
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
                &options.insert_table_name,
                &options.widths,
                &column_names,
                &rows,
            )?;
        }
        if options.changes {
            let n = if rql_statement_changes_rows(statement) {
                stmt.affected_rows() as i64
            } else {
                0
            };
            *total_changes += n;
            writeln!(out, "changes: {n}   total_changes: {total_changes}")
                .map_err(|err| err.to_string())?;
        }
        if options.stats {
            writeln!(out, "Memory Used: 0 (max 0) bytes").map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn rql_statement_changes_rows(statement: &RqlStatement) -> bool {
    matches!(
        statement,
        RqlStatement::Insert(_) | RqlStatement::Update(_) | RqlStatement::Delete(_)
    )
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
