//! Schema-introspection dot-commands: `.tables`, `.schema`, `.indexes`,
//! `.databases`.

use redlinedb::Step;

use super::{CliState, DotOutcome};

/// `.tables [PATTERN]` — list tables (and views) whose name matches the
/// optional LIKE pattern.
pub fn tables(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let pattern = args.first().copied().unwrap_or("%");
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master \
             WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
             AND name LIKE ?1 ORDER BY name",
        )
        .map_err(|err| err.to_string())?;
    stmt.bind_text(1, pattern).map_err(|err| err.to_string())?;
    let mut names: Vec<String> = Vec::new();
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let name: String = row.get(0).map_err(|err| err.to_string())?;
        names.push(name);
    }
    if names.is_empty() {
        return Ok(DotOutcome::Ok);
    }
    // sqlite3 packs table names column-major (top-to-bottom, then left-to-right)
    // with each column sized to the widest name in that column and a 5-space
    // gap between adjacent columns. The number of columns is computed against
    // an 80-column terminal using the global maximum name length.
    let max_name = names.iter().map(|n| n.chars().count()).max().unwrap_or(1);
    const TERMINAL: usize = 80;
    const SEP: usize = 5;
    let cols = ((TERMINAL + SEP) / (max_name + SEP)).max(1);
    let total = names.len();
    let rows = total.div_ceil(cols);
    // Per-column widths driven by the longest name placed in that column.
    let mut col_widths = vec![0usize; cols];
    for col in 0..cols {
        for row in 0..rows {
            let idx = col * rows + row;
            if let Some(name) = names.get(idx) {
                col_widths[col] = col_widths[col].max(name.chars().count());
            }
        }
    }
    for row in 0..rows {
        let mut line = String::new();
        for col in 0..cols {
            let idx = col * rows + row;
            if idx >= total {
                continue;
            }
            if !line.is_empty() {
                for _ in 0..SEP {
                    line.push(' ');
                }
            }
            line.push_str(&format!("{:<width$}", names[idx], width = col_widths[col]));
        }
        state
            .output
            .write_line(line.trim_end())
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.schema [TABLE]` — print stored `CREATE` statements. Pattern is matched
/// against the object name with LIKE.
///
/// Recognises the sqlite3 shell flags `--indent` and `--nosys`. `--indent` is
/// honoured by stripping the flag (the underlying CREATE text is emitted
/// verbatim; sqlite3 only re-formats CREATEs that were not stored with
/// indentation, which is fine to no-op here). `--nosys` is the default
/// behaviour and is accepted for compatibility.
pub fn schema(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let (flags, positional): (Vec<&str>, Vec<&str>) =
        args.iter().partition(|arg| arg.starts_with("--"));
    let _ = flags; // accepted-and-ignored: `--indent`, `--nosys`
    let pattern = positional.first().copied().unwrap_or("%");

    // sqlite3's `.schema sqlite_master` (or the historical alias
    // `sqlite_schema`) prints the canonical schema for the system table.
    if pattern.eq_ignore_ascii_case("sqlite_master")
        || pattern.eq_ignore_ascii_case("sqlite_schema")
    {
        for line in [
            "CREATE TABLE sqlite_schema (",
            "  type text,",
            "  name text,",
            "  tbl_name text,",
            "  rootpage integer,",
            "  sql text",
            ");",
        ] {
            state
                .output
                .write_line(line)
                .map_err(|err| err.to_string())?;
        }
        return Ok(DotOutcome::Ok);
    }

    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    // sqlite3 emits schema entries in creation order. redlinedb's
    // sqlite_master doesn't expose `rowid`, so we leave ORDER BY off and
    // rely on the catalog returning rows in insertion order.
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' AND name LIKE ?1 AND sql IS NOT NULL",
        )
        .map_err(|err| err.to_string())?;
    stmt.bind_text(1, pattern).map_err(|err| err.to_string())?;
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let sql: String = row.get(0).map_err(|err| err.to_string())?;
        let line = if sql.ends_with(';') {
            sql
        } else {
            format!("{sql};")
        };
        state
            .output
            .write_line(&line)
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.indexes [TABLE]` — list indexes optionally filtered by table.
pub fn indexes(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let (sql, bind_value): (&str, Option<&str>) = match args.first() {
        Some(table) => (
            "SELECT name FROM sqlite_master \
             WHERE type='index' AND tbl_name = ?1 ORDER BY name",
            Some(*table),
        ),
        None => (
            "SELECT name FROM sqlite_master WHERE type='index' ORDER BY name",
            None,
        ),
    };
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    if let Some(value) = bind_value {
        stmt.bind_text(1, value).map_err(|err| err.to_string())?;
    }
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let name: String = row.get(0).map_err(|err| err.to_string())?;
        state
            .output
            .write_line(&name)
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.databases` — list attached databases.
pub fn databases(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    let mut stmt = state
        .conn
        .prepare("PRAGMA database_list")
        .map_err(|err| err.to_string())?;
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let name: String = row.get(1).map_err(|err| err.to_string())?;
        let path: String = row.get(2).map_err(|err| err.to_string())?;
        let path = if path == ":memory:" {
            String::new()
        } else {
            path
        };
        state
            .output
            .write_line(&format!("{name}: \"{path}\" r/w"))
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}

/// `.fullschema [PATTERN]` — like `.schema` but also dumps the contents of
/// `sqlite_master` so callers can see the raw catalog rows (type, name,
/// tbl_name, rootpage, sql).
pub fn fullschema(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    schema(state, args)?;
    state
        .output
        .write_line("/* sqlite_master */")
        .map_err(|err| err.to_string())?;
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT type, name, tbl_name, rootpage, sql FROM sqlite_master \
             ORDER BY type, name",
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
        let line = format!("{ty}|{name}|{tbl_name}|{rootpage}|{sql}");
        state
            .output
            .write_line(&line)
            .map_err(|err| err.to_string())?;
    }
    Ok(DotOutcome::Ok)
}
