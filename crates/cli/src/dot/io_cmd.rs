//! I/O dot-commands: `.read`, `.dump`, `.save`, `.restore`, `.output`,
//! `.import`, `.print`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use redlinedb::{
    BackupOptions, Database, OpenOptions as DbOpenOptions, OwnedStatement, OwnedStep,
    RestoreOptions, Step, ValueRef,
};

use super::{CliState, DotOutcome, OutputTarget};

/// `.output FILE|stdout` — redirect query output. When called with no
/// arguments, restores stdout.
pub fn output(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    match args.first().copied() {
        None | Some("stdout") => {
            state.output.flush().map_err(|err| err.to_string())?;
            state.output = OutputTarget::stdout();
        }
        Some("off") => {
            state.output.flush().map_err(|err| err.to_string())?;
            state.output = OutputTarget::Null;
        }
        Some(path) => {
            state.output.flush().map_err(|err| err.to_string())?;
            let path_buf = PathBuf::from(path);
            let writer = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path_buf)
                .map_err(|err| format!("Error: cannot open {path}: {err}"))?;
            state.output = OutputTarget::file(path_buf, writer);
        }
    }
    Ok(DotOutcome::Ok)
}

/// `.open FILE` — reopen the shell on a new database. `:memory:` creates a
/// fresh private transient database.
pub fn open(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(path) = args.first() else {
        return Err("Error: usage: .open FILENAME".to_owned());
    };
    let snapshot_path = PathBuf::from(path);
    if let Some(db) = state.snapshots.get(&snapshot_path).cloned() {
        state.db = db;
        state.db_path = snapshot_path;
        state.reconnect()?;
        return Ok(DotOutcome::Ok);
    }
    let (db, db_path) = open_shell_database(path)?;
    state.db = db;
    state.db_path = db_path;
    state.reconnect()?;
    Ok(DotOutcome::Ok)
}

/// `.once FILE` — redirect the next single statement's output to `FILE`.
pub fn once(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    match args.first().copied() {
        None => {
            state.once = None;
        }
        Some(path) => {
            state.once = Some(PathBuf::from(path));
        }
    }
    Ok(DotOutcome::Ok)
}

/// `.print TEXT...` — emit literal text followed by a newline.
pub fn print(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let line = args.join(" ");
    state
        .output
        .write_line(&line)
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

/// `.read FILENAME` — hand the path back to the REPL so it can stream the
/// file through the SQL runner.
pub fn read(_state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(path) = args.first() else {
        return Err("Error: usage: .read FILENAME".to_owned());
    };
    Ok(DotOutcome::ReadFile(PathBuf::from(path)))
}

/// `.backup ?DB? FILE` — write the current RedlineDB image to FILE. The
/// optional schema argument is accepted for SQLite shell compatibility.
pub fn backup(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let path = match args {
        [path] => *path,
        [_schema, path] => *path,
        _ => return Err("Error: usage: .backup ?DB? FILE".to_owned()),
    };
    state
        .db
        .backup_to_path(PathBuf::from(path), BackupOptions::default())
        .map_err(|err| format!("Error: {err}"))?;
    state
        .snapshots
        .insert(PathBuf::from(path), state.db.clone());
    Ok(DotOutcome::Ok)
}

/// `.save FILE` — copy the current database to FILE via the logical backup
/// API. Matches the SQLite shell's behaviour of producing a stand-alone
/// snapshot of the database in the engine's native format.
pub fn save(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(path) = args.first() else {
        return Err("Error: usage: .save FILE".to_owned());
    };
    state
        .db
        .backup_to_path(PathBuf::from(path), BackupOptions::default())
        .map_err(|err| format!("Error: {err}"))?;
    state
        .snapshots
        .insert(PathBuf::from(path), state.db.clone());
    Ok(DotOutcome::Ok)
}

/// `.restore ?DB? FILE` — reopen on a backup produced by `.backup`, `.save`, or
/// `.clone`. The optional schema argument is accepted and ignored.
pub fn restore(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let src = match args {
        [src] => *src,
        [_schema, src] => *src,
        _ => return Err("Error: usage: .restore ?DB? FILE".to_owned()),
    };
    let src_path = PathBuf::from(src);
    if let Some(db) = state.snapshots.get(&src_path).cloned() {
        state.db = db;
        state.db_path = src_path;
        state.reconnect()?;
        return Ok(DotOutcome::Ok);
    }
    match Database::open(&src_path) {
        Ok(db) => {
            state.db = db;
            state.db_path = src_path;
            state.reconnect()?;
        }
        Err(open_err) => {
            let dst = state.db_path.clone();
            Database::restore_from_backup(&src_path, dst.clone(), RestoreOptions::default())
                .map_err(|err| format!("Error: {err}; open backup failed: {open_err}"))?;
            state.db = Database::open(&dst).map_err(|err| format!("Error: {err}"))?;
            state.reconnect()?;
        }
    }
    Ok(DotOutcome::Ok)
}

/// `.clone FILE` — copy the current database image and leave the shell on the
/// original connection.
pub fn clone_db(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(path) = args.first() else {
        return Err("Error: usage: .clone FILE".to_owned());
    };
    state
        .db
        .backup_to_path(PathBuf::from(path), BackupOptions::default())
        .map_err(|err| format!("Error: {err}"))?;
    state
        .snapshots
        .insert(PathBuf::from(path), state.db.clone());
    Ok(DotOutcome::Ok)
}

/// `.dbinfo` — print basic database information.
pub fn dbinfo(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare("PRAGMA page_size")
        .map_err(|err| err.to_string())?;
    let mut page_size = None;
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        page_size = Some(row.get::<i64>(0).map_err(|err| err.to_string())?);
    }
    let value = page_size.unwrap_or(0);
    state
        .output
        .write_line(&format!("database page size: {value}"))
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

/// `.dbtotxt` — dump a simple text view of sqlite_master rows.
pub fn dbtotxt(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT type, name, tbl_name, rootpage, sql FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .map_err(|err| err.to_string())?;
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let ty: String = row.get(0).map_err(|err| err.to_string())?;
        let name: String = row.get(1).map_err(|err| err.to_string())?;
        let tbl_name: String = row.get(2).map_err(|err| err.to_string())?;
        let rootpage: i64 = row.get(3).unwrap_or(0);
        let sql = match row.get::<Option<String>>(4) {
            Ok(Some(sql)) => sql,
            Ok(None) => String::new(),
            Err(err) => return Err(err.to_string()),
        };
        state
            .output
            .write_line(&format!("{ty}|{name}|{tbl_name}|{rootpage}|{sql}"))
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.recover` — recover by emitting a dump of the database contents.
pub fn recover(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    dump(state, args)
}

/// `.cd DIR` — change the process working directory.
pub fn cd(_state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(path) = args.first() else {
        return Err("Error: usage: .cd DIRECTORY".to_owned());
    };
    std::env::set_current_dir(path).map_err(|err| format!("Error: cannot cd to {path}: {err}"))?;
    Ok(DotOutcome::Ok)
}

/// `.import FILE TABLE` — load CSV rows from FILE into TABLE.
///
/// Wave 1 already hoisted the prepared statement out of the per-row loop
/// and wrapped the inserts in a single transaction. When the session
/// flag `PRAGMA redline_bulk_import = ON` is set, this routine takes a
/// further batched path: it groups CSV rows in chunks of
/// `BULK_IMPORT_CHUNK` and submits each chunk as a single multi-row
/// `INSERT INTO t VALUES (?,?,..),(?,?,..),...` statement. The
/// executor's `with_write_tx` is then entered once per chunk instead of
/// once per row, while still going through the existing parser/planner
/// and constraint machinery (no engine boundary crossed).
pub fn import(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let filtered = args
        .iter()
        .copied()
        .filter(|arg| !matches!(*arg, "--csv" | "-csv"))
        .collect::<Vec<_>>();
    let args = filtered.as_slice();
    if args.len() < 2 {
        return Err("Error: usage: .import FILE TABLE".to_owned());
    }
    let path = args[0];
    let table = args[1];
    let file = File::open(path).map_err(|err| format!("Error: cannot open {path}: {err}"))?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(file);
    let bulk_enabled = bulk_import_enabled(state);
    // Wrap the row loop in a transaction so N inserts pay one WAL group commit
    // instead of N; prepare once so we avoid re-parse+plan per row.
    state
        .conn
        .execute("BEGIN", ())
        .map_err(|err| err.to_string())?;
    let result = if bulk_enabled {
        import_rows_batched(state, &mut reader, table)
    } else {
        let mut header: Option<Vec<String>> = None;
        let mut stmt: Option<OwnedStatement> = None;
        let res = import_rows(state, &mut reader, &mut header, &mut stmt, table);
        drop(stmt);
        res
    };
    match result {
        Ok(()) => {
            state
                .conn
                .execute("COMMIT", ())
                .map_err(|err| err.to_string())?;
            Ok(DotOutcome::Ok)
        }
        Err(err) => {
            let _ = state.conn.execute("ROLLBACK", ());
            Err(err)
        }
    }
}

/// Probe `PRAGMA redline_bulk_import` on the active connection. Any
/// failure (PRAGMA not supported, no row) is treated as OFF so the
/// caller falls through to the default path.
fn bulk_import_enabled(state: &mut CliState) -> bool {
    let mut stmt = match state.conn.prepare("PRAGMA redline_bulk_import") {
        Ok(s) => s,
        Err(_) => return false,
    };
    match stmt.step() {
        Ok(Step::Row(row)) => row.get::<i64>(0).map(|v| v != 0).unwrap_or(false),
        _ => false,
    }
}

/// Rows-per-batched-INSERT. SQLite uses 1000 for `.mode insert` and the
/// host parser's parameter cap (32 766) limits any practical row arity
/// at this chunk size.
const BULK_IMPORT_CHUNK: usize = 1000;

fn import_rows_batched(
    state: &mut CliState,
    reader: &mut csv::Reader<File>,
    table: &str,
) -> Result<(), String> {
    let mut header: Option<Vec<String>> = None;
    let mut columns: Option<usize> = None;
    let mut full_stmt: Option<OwnedStatement> = None;
    let mut buffer: Vec<csv::StringRecord> = Vec::with_capacity(BULK_IMPORT_CHUNK);

    for record in reader.records() {
        let record = record.map_err(|err| format!("Error: {err}"))?;
        if header.is_none() && state.show_header {
            *(&mut header) = Some(record.iter().map(str::to_owned).collect());
            continue;
        }
        if columns.is_none() {
            let n = header
                .as_ref()
                .map(|h| h.len())
                .unwrap_or_else(|| record.len());
            columns = Some(n);
        }
        buffer.push(record);
        if buffer.len() >= BULK_IMPORT_CHUNK {
            let cols = columns.expect("columns initialised above");
            flush_chunk(state, &mut full_stmt, header.as_ref(), table, cols, &buffer)?;
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        let cols = columns.expect("columns initialised when first record observed");
        // Final chunk may be smaller than BULK_IMPORT_CHUNK; build a
        // one-shot statement for the trailing rows rather than reusing
        // the full-width prepared statement.
        let sql = build_batched_insert_sql(table, header.as_ref(), cols, buffer.len());
        let mut stmt = state
            .conn
            .prepare_owned(&sql)
            .map_err(|err| err.to_string())?;
        bind_chunk(&mut stmt, &buffer, cols)?;
        while let OwnedStep::Row = stmt.step().map_err(|err| err.to_string())? {}
    }
    Ok(())
}

fn flush_chunk(
    state: &mut CliState,
    full_stmt: &mut Option<OwnedStatement>,
    header: Option<&Vec<String>>,
    table: &str,
    cols: usize,
    buffer: &[csv::StringRecord],
) -> Result<(), String> {
    if full_stmt.is_none() {
        let sql = build_batched_insert_sql(table, header, cols, BULK_IMPORT_CHUNK);
        *full_stmt = Some(
            state
                .conn
                .prepare_owned(&sql)
                .map_err(|err| err.to_string())?,
        );
    }
    let stmt = full_stmt.as_mut().expect("statement initialised above");
    stmt.reset().map_err(|err| err.to_string())?;
    stmt.clear_bindings();
    bind_chunk(stmt, buffer, cols)?;
    while let OwnedStep::Row = stmt.step().map_err(|err| err.to_string())? {}
    Ok(())
}

fn bind_chunk(
    stmt: &mut OwnedStatement,
    buffer: &[csv::StringRecord],
    cols: usize,
) -> Result<(), String> {
    let mut slot = 1usize;
    for record in buffer {
        for c in 0..cols {
            let field = record.get(c).unwrap_or("");
            stmt.bind_text(slot, field).map_err(|err| err.to_string())?;
            slot += 1;
        }
    }
    Ok(())
}

fn build_batched_insert_sql(
    table: &str,
    header: Option<&Vec<String>>,
    cols: usize,
    rows: usize,
) -> String {
    let one_row = {
        let mut s = String::with_capacity(cols * 2 + 2);
        s.push('(');
        for i in 0..cols {
            if i > 0 {
                s.push(',');
            }
            s.push('?');
        }
        s.push(')');
        s
    };
    let mut values = String::with_capacity(rows * (one_row.len() + 1));
    for i in 0..rows {
        if i > 0 {
            values.push(',');
        }
        values.push_str(&one_row);
    }
    if let Some(h) = header {
        let col_list = h
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(",");
        format!("INSERT INTO {table}({col_list}) VALUES {values}")
    } else {
        format!("INSERT INTO {table} VALUES {values}")
    }
}

fn import_rows(
    state: &mut CliState,
    reader: &mut csv::Reader<File>,
    header: &mut Option<Vec<String>>,
    stmt: &mut Option<OwnedStatement>,
    table: &str,
) -> Result<(), String> {
    for record in reader.records() {
        let record = record.map_err(|err| format!("Error: {err}"))?;
        if header.is_none() && state.show_header {
            *header = Some(record.iter().map(str::to_owned).collect());
            continue;
        }
        if stmt.is_none() {
            let columns = match header.as_ref() {
                Some(h) => h.len(),
                None => record.len(),
            };
            let placeholders = std::iter::repeat_n("?", columns)
                .collect::<Vec<_>>()
                .join(",");
            let sql = if let Some(h) = header.as_ref() {
                let cols = h
                    .iter()
                    .map(|c| quote_ident(c))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("INSERT INTO {table}({cols}) VALUES ({placeholders})")
            } else {
                format!("INSERT INTO {table} VALUES ({placeholders})")
            };
            *stmt = Some(
                state
                    .conn
                    .prepare_owned(&sql)
                    .map_err(|err| err.to_string())?,
            );
        }
        let prepared = stmt.as_mut().expect("statement initialised above");
        prepared.reset().map_err(|err| err.to_string())?;
        prepared.clear_bindings();
        for (i, field) in record.iter().enumerate() {
            prepared
                .bind_text(i + 1, field)
                .map_err(|err| err.to_string())?;
        }
        while let OwnedStep::Row = prepared.step().map_err(|err| err.to_string())? {}
    }
    Ok(())
}

/// `.dump [TABLE]` — serialise the database (or one table) to SQLite-shell
/// compatible text on the active output sink.
pub fn dump(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let table_filter = args.first().copied();
    state
        .output
        .write_line("BEGIN TRANSACTION;")
        .map_err(|err| err.to_string())?;
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let select_sql = if table_filter.is_some() {
        "SELECT type, name, tbl_name, sql FROM sqlite_master \
         WHERE name = ?1 AND type = 'table'"
    } else {
        "SELECT type, name, tbl_name, sql FROM sqlite_master \
         WHERE type IN ('table','index','view','trigger') \
           AND name NOT LIKE 'sqlite_%' \
         ORDER BY CASE type WHEN 'table' THEN 1 WHEN 'index' THEN 2 \
                            WHEN 'view' THEN 3 WHEN 'trigger' THEN 4 END, name"
    };
    let mut stmt = conn.prepare(select_sql).map_err(|err| err.to_string())?;
    if let Some(filter) = table_filter {
        stmt.bind_text(1, filter).map_err(|err| err.to_string())?;
    }
    let mut tables: Vec<(String, String)> = Vec::new();
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let obj_type: String = row.get(0).map_err(|err| err.to_string())?;
        let name: String = row.get(1).map_err(|err| err.to_string())?;
        let sql: String = row.get(3).map_err(|err| err.to_string())?;
        let trimmed = if sql.ends_with(';') {
            sql.clone()
        } else {
            format!("{sql};")
        };
        state
            .output
            .write_line(&trimmed)
            .map_err(|err| err.to_string())?;
        if obj_type == "table" {
            tables.push((name, sql));
        }
    }
    drop(stmt);
    for (name, _create_sql) in &tables {
        dump_table_rows(state, &mut conn, name)?;
    }
    state
        .output
        .write_line("COMMIT;")
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}

fn dump_table_rows(
    state: &mut CliState,
    conn: &mut redlinedb::Connection,
    table: &str,
) -> Result<(), String> {
    let select_sql = format!("SELECT * FROM {}", quote_ident(table));
    let mut stmt = conn.prepare(&select_sql).map_err(|err| err.to_string())?;
    let column_count = stmt.column_count();
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let mut values: Vec<String> = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let formatted = match row.get_ref(i).map_err(|err| err.to_string())? {
                ValueRef::Null => "NULL".to_owned(),
                ValueRef::Integer(v) => v.to_string(),
                ValueRef::Real(v) => format_real(v),
                ValueRef::Text(v) => quote_string(v),
                ValueRef::Blob(v) => quote_blob(v),
            };
            values.push(formatted);
        }
        let line = format!(
            "INSERT INTO {} VALUES({});",
            dump_ident(table),
            values.join(",")
        );
        state
            .output
            .write_line(&line)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn quote_ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn dump_ident(name: &str) -> String {
    if is_simple_ident(name) {
        name.to_owned()
    } else {
        quote_ident(name)
    }
}

fn is_simple_ident(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn open_shell_database(path: &str) -> Result<(Database, PathBuf), String> {
    if path == ":memory:" || path.is_empty() {
        let db = Database::create_in_memory(DbOpenOptions::default())
            .map_err(|err| format!("Error: {err}"))?;
        Ok((db, PathBuf::from(":memory:")))
    } else {
        let db = Database::open(path).map_err(|err| format!("Error: {err}"))?;
        Ok((db, PathBuf::from(path)))
    }
}

fn quote_string(value: &str) -> String {
    let escaped = value.replace('\'', "''");
    format!("'{escaped}'")
}

fn quote_blob(value: &[u8]) -> String {
    let mut out = String::with_capacity(value.len() * 2 + 3);
    out.push_str("X'");
    for byte in value {
        out.push_str(&format!("{byte:02X}"));
    }
    out.push('\'');
    out
}

fn format_real(value: f64) -> String {
    // Delegate to the SQL crate's `format_real_sqlite` so `.dump` / `.iotrace`
    // output matches SQLite's reference shell `%!.17g` semantics.
    redlinedb::format_real_sqlite(value)
}
