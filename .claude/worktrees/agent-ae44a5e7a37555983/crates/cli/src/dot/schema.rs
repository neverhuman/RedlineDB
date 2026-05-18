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
    // SQLite prints names left-aligned in columns sized to the longest.
    let width = names.iter().map(|n| n.len()).max().unwrap_or(0).max(1);
    let cols = (80 / (width + 2)).max(1);
    let mut line = String::new();
    for (i, name) in names.iter().enumerate() {
        line.push_str(&format!("{name:<width$}", width = width));
        if (i + 1) % cols == 0 || i + 1 == names.len() {
            state
                .output
                .write_line(line.trim_end())
                .map_err(|err| err.to_string())?;
            line.clear();
        } else {
            line.push_str("  ");
        }
    }
    Ok(DotOutcome::Ok)
}

/// `.schema [TABLE]` — print stored `CREATE` statements. Pattern is matched
/// against the object name with LIKE.
pub fn schema(state: &mut CliState, args: &[&str]) -> Result<DotOutcome, String> {
    let pattern = args.first().copied().unwrap_or("%");
    let mut conn = state.db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' AND name LIKE ?1 AND sql IS NOT NULL \
             ORDER BY type, name",
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

/// `.databases` — list attached databases (currently just `main`).
pub fn databases(state: &mut CliState, _args: &[&str]) -> Result<DotOutcome, String> {
    let line = format!("main: {}", state.db_path.display());
    state
        .output
        .write_line(&line)
        .map_err(|err| err.to_string())?;
    Ok(DotOutcome::Ok)
}
