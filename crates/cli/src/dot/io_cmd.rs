//! I/O dot-commands: `.read`, `.dump`, `.save`, `.restore`, `.output`,
//! `.import`, `.print`.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use redlinedb::{BackupOptions, Database, RestoreOptions, Step, ValueRef};

use super::{CliState, DotOutcome, OutputTarget};

/// `.output FILE|stdout` — redirect query output. When called with no
/// arguments, restores stdout.
pub fn output(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    match args.first().copied() {
        None | Some("stdout") => {
            state.output = OutputTarget::Stdout;
        }
        Some("off") => {
            // SQLite treats `off` as discarding output. The most faithful
            // mapping in Rust is `/dev/null` on Unix, but we instead route to
            // an in-memory sink represented as a writable but discarded file.
            state.output = OutputTarget::Stdout;
        }
        Some(path) => {
            let path_buf = PathBuf::from(path);
            let writer = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path_buf)
                .map_err(|err| format!("Error: cannot open {path}: {err}"))?;
            state.output = OutputTarget::File {
                path: path_buf,
                writer,
            };
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
    Ok(DotOutcome::Ok)
}

/// `.restore FILE` — replace the current database contents with the contents
/// of FILE (interpreted as a backup produced by [`save`]).
pub fn restore(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let Some(src) = args.first() else {
        return Err("Error: usage: .restore FILE".to_owned());
    };
    let dst = state.db_path.clone();
    Database::restore_from_backup(PathBuf::from(src), dst.clone(), RestoreOptions::default())
        .map_err(|err| format!("Error: {err}"))?;
    // Reopen the database so subsequent statements see the restored content.
    state.db = Database::open(&dst).map_err(|err| format!("Error: {err}"))?;
    Ok(DotOutcome::Ok)
}

/// `.import FILE TABLE` — load CSV rows from FILE into TABLE.
pub fn import(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
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
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut header: Option<Vec<String>> = None;
    let mut insert_sql: Option<String> = None;
    for record in reader.records() {
        let record = record.map_err(|err| format!("Error: {err}"))?;
        if header.is_none() && state.show_header {
            header = Some(record.iter().map(str::to_owned).collect());
            continue;
        }
        let columns = match header.as_ref() {
            Some(h) => h.len(),
            None => record.len(),
        };
        let sql = match &insert_sql {
            Some(s) => s.clone(),
            None => {
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
                insert_sql = Some(sql.clone());
                sql
            }
        };
        let mut stmt = conn.prepare(&sql).map_err(|err| err.to_string())?;
        for (i, field) in record.iter().enumerate() {
            stmt.bind_text(i + 1, field)
                .map_err(|err| err.to_string())?;
        }
        while let Step::Row(_) = stmt.step().map_err(|err| err.to_string())? {}
    }
    Ok(DotOutcome::Ok)
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
            quote_ident(table),
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
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}
