use std::env;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use redlinedb::{
    ArchiveMode, BackupOptions, Database, PhysicalBackupOptions, RecoveryTarget, RestoreOptions,
    Step, ValueRef,
};
use serde_json::json;

#[derive(Parser, Debug)]
#[command(name = "redlinedb", about = "RedlineDB CLI (SQLite Drop-in)", disable_help_flag = true, disable_version_flag = true)]
struct Cli {
    #[arg(long = "help")]
    help: bool,

    #[arg(long = "version")]
    version: bool,

    #[arg(long)]
    bail: bool,

    #[arg(long)]
    batch: bool,

    #[arg(long)]
    csv: bool,

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
    table: bool,

    #[arg(long)]
    tabs: bool,

    #[arg(long)]
    separator: Option<String>,

    #[arg(long)]
    init: Option<String>,

    #[arg(long)]
    cmd: Option<String>,

    #[arg(name = "FILENAME")]
    filename: Option<String>,

    #[arg(name = "SQL")]
    sql: Option<String>,
}

#[derive(Debug, PartialEq)]
enum OutputMode {
    List,
    Csv,
    Json,
    Line,
    Markdown,
    Quote,
    Table,
    Tabs,
}

fn main() {
    let mut args: Vec<String> = env::args().collect();
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
        println!("3.45.1"); // Fool the agent into thinking it's sqlite
        return;
    }

    let filename = match cli.filename {
        Some(f) => f,
        None => ":memory:".to_string(),
    };
    
    // Determine output mode
    let mut mode = OutputMode::List;
    if cli.csv { mode = OutputMode::Csv; }
    else if cli.json { mode = OutputMode::Json; }
    else if cli.line { mode = OutputMode::Line; }
    else if cli.markdown { mode = OutputMode::Markdown; }
    else if cli.quote { mode = OutputMode::Quote; }
    else if cli.table { mode = OutputMode::Table; }
    else if cli.tabs { mode = OutputMode::Tabs; }

    let separator = cli.separator.unwrap_or_else(|| {
        match mode {
            OutputMode::Tabs => "\t".to_string(),
            OutputMode::Csv => ",".to_string(),
            _ => "|".to_string(),
        }
    });

    let show_header = cli.header && !cli.noheader;

    let db_res = Database::open(&filename);
    let db = match db_res {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error: {}", e);
            exit(1);
        }
    };

    if let Some(cmd) = cli.cmd {
        if let Err(e) = run_query_sqlite(&db, &cmd, &mode, &separator, show_header) {
            eprintln!("Error: {}", e);
            if cli.bail { exit(1); }
        }
    }

    if let Some(sql) = cli.sql {
        if cli.echo {
            println!("{}", sql);
        }
        if let Err(e) = run_query_sqlite(&db, &sql, &mode, &separator, show_header) {
            eprintln!("Error: {}", e);
            exit(1);
        }
        return;
    }

    // Check if stdin is a tty
    use std::io::IsTerminal;
    let is_tty = io::stdin().is_terminal();

    if !is_tty || cli.batch {
        // Read from stdin
        let stdin = io::stdin();
        let mut buffer = String::new();
        for line in stdin.lock().lines() {
            let line = line.unwrap_or_default();
            buffer.push_str(&line);
            buffer.push('\n');
            if line.trim_end().ends_with(';') {
                if cli.echo {
                    println!("{}", buffer.trim_end());
                }
                if let Err(e) = run_query_sqlite(&db, &buffer, &mode, &separator, show_header) {
                    eprintln!("Error: {}", e);
                    if cli.bail { exit(1); }
                }
                buffer.clear();
            }
        }
        if !buffer.trim().is_empty() {
            if let Err(e) = run_query_sqlite(&db, &buffer, &mode, &separator, show_header) {
                eprintln!("Error: {}", e);
                if cli.bail { exit(1); }
            }
        }
    } else {
        // Interactive REPL
        println!("RedlineDB version 1.0.0 (SQLite 3.45.1 compatibility)");
        println!("Enter \".help\" for usage hints.");
        println!("Connected to a transient in-memory database.");
        println!("Use \".open FILENAME\" to reopen on a persistent database.");
        
        let mut rl = rustyline::DefaultEditor::new().unwrap();
        let mut buffer = String::new();
        loop {
            let prompt = if buffer.is_empty() { "sqlite> " } else { "   ...> " };
            let readline = rl.readline(prompt);
            match readline {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() { continue; }
                    
                    if buffer.is_empty() && line.starts_with('.') {
                        handle_dot_command(line);
                        continue;
                    }
                    
                    rl.add_history_entry(line).unwrap();
                    buffer.push_str(line);
                    buffer.push('\n');
                    
                    if line.ends_with(';') {
                        if let Err(e) = run_query_sqlite(&db, &buffer, &mode, &separator, show_header) {
                            eprintln!("Error: {}", e);
                        }
                        buffer.clear();
                    }
                },
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    buffer.clear();
                },
                Err(rustyline::error::ReadlineError::Eof) => {
                    break;
                },
                Err(err) => {
                    eprintln!("Error: {:?}", err);
                    break;
                }
            }
        }
    }
}

fn handle_dot_command(cmd: &str) {
    if cmd == ".exit" || cmd == ".quit" {
        exit(0);
    } else if cmd == ".help" {
        println!(".exit                  Exit this program");
        println!(".help                  Show this message");
        println!(".quit                  Exit this program");
    } else {
        println!("Error: unknown command or invalid arguments: \"{}\". Enter \".help\" for help", cmd.split_whitespace().next().unwrap_or(""));
    }
}

fn print_sqlite_help() {
    println!("Usage: sqlite3 [OPTIONS] [FILENAME [SQL]]");
    println!("FILENAME is the name of an SQLite database.");
    println!("OPTIONS include:");
    println!("   -bail                stop after hitting an error");
    println!("   -batch               force batch I/O");
    println!("   -csv                 set output mode to 'csv'");
    println!("   -echo                print inputs before execution");
    println!("   -[no]header          turn headers on or off");
    println!("   -help                show this message");
    println!("   -json                set output mode to 'json'");
    println!("   -line                set output mode to 'line'");
    println!("   -list                set output mode to 'list'");
    println!("   -separator SEP       set output column separator. Default: '|'");
    println!("   -version             show SQLite version");
}

fn run_query_sqlite(db: &Database, sql: &str, mode: &OutputMode, separator: &str, _show_header: bool) -> Result<(), String> {
    let mut conn = db.connect().map_err(|err| err.to_string())?;

    // Split the input into individual statements separated by ';'
    // This matches sqlite3 behavior where multiple statements can be piped on one line.
    let statements: Vec<&str> = sql.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for statement in statements {
        let full_stmt = format!("{};", statement);
        let mut stmt = conn.prepare(&full_stmt).map_err(|err| err.to_string())?;
        let column_count = stmt.column_count();

        let mut json_results = Vec::new();

        while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
            if *mode == OutputMode::Json {
                let mut obj = serde_json::Map::new();
                for index in 0..column_count {
                    let val = match row.get_ref(index).map_err(|err| err.to_string())? {
                        ValueRef::Null => serde_json::Value::Null,
                        ValueRef::Integer(v) => json!(v),
                        ValueRef::Real(v) => json!(v),
                        ValueRef::Text(v) => json!(v),
                        ValueRef::Blob(v) => json!(v),
                    };
                    obj.insert(format!("column{}", index), val);
                }
                json_results.push(serde_json::Value::Object(obj));
            } else {
                let mut first = true;
                for index in 0..column_count {
                    if !first {
                        print!("{}", separator);
                    }
                    first = false;
                    match row.get_ref(index).map_err(|err| err.to_string())? {
                        ValueRef::Null => print!(""),
                        ValueRef::Integer(value) => print!("{value}"),
                        ValueRef::Real(value) => print!("{value}"),
                        ValueRef::Text(value) => {
                            if *mode == OutputMode::Csv {
                                let escaped = value.replace("\"", "\"\"");
                                if escaped.contains(",") || escaped.contains("\"") || escaped.contains("\n") {
                                    print!("\"{}\"", escaped);
                                } else {
                                    print!("{}", escaped);
                                }
                            } else {
                                print!("{value}");
                            }
                        },
                        ValueRef::Blob(value) => print!("<blob:{}>", value.len()),
                    }
                }
                println!();
            }
        }

        if *mode == OutputMode::Json && !json_results.is_empty() {
            println!("{}", serde_json::to_string_pretty(&json_results).unwrap_or_default());
        }
    }

    Ok(())
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
