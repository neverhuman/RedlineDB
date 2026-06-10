use super::*;

pub(crate) fn pragma_table_info_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    pragma_column_rows(table, false)
}

pub(crate) fn pragma_table_xinfo_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    pragma_column_rows(table, true)
}

fn pragma_column_rows(
    table: &redlinedb_kernel::catalog::TableDef,
    include_hidden: bool,
) -> Vec<Vec<SqlValue>> {
    let mut pk = vec![0_i64; table.columns.len()];
    if let Some(ordinal) = table.rowid_alias_column {
        if let Some(slot) = pk.get_mut(ordinal as usize) {
            *slot = 1;
        }
    } else if let Some(index) = table.indexes.iter().find(|index| index.primary) {
        for (position, key) in index.keys.iter().enumerate() {
            let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
                continue;
            };
            if let Some(slot) = pk.get_mut(*attnum as usize) {
                *slot = (position + 1) as i64;
            }
        }
    }

    let rowid_alias_ordinal = table.rowid_alias_column;
    // Index user-explicit NOT NULL declarations so we can distinguish
    // "kernel set not_null because of PRIMARY KEY" (surface notnull = 0
    // for a rowid alias) from "user wrote NOT NULL" (surface notnull = 1).
    use std::collections::HashSet;
    let explicit_not_null = table
        .constraints
        .iter()
        .any(|c| matches!(c.kind, redlinedb_kernel::catalog::ConstraintKind::NotNull))
        .then(|| {
            table
                .constraints
                .iter()
                .filter(|c| matches!(c.kind, redlinedb_kernel::catalog::ConstraintKind::NotNull))
                .filter_map(|c| {
                    let column_id = c.column_id?;
                    table
                        .columns
                        .iter()
                        .position(|col| col.column_id == column_id)
                        .map(|ordinal| ordinal as u16)
                })
                .collect::<HashSet<u16>>()
        });
    table
        .columns
        .iter()
        .enumerate()
        .map(|(cid, column)| {
            // SQLite reports `notnull = 0` for a rowid-alias column
            // because the prohibition on NULL is a consequence of the
            // rowid mechanic, not an explicit declaration. We honour
            // that surface: only flag `notnull = 1` when the user
            // wrote NOT NULL explicitly OR the column is not the
            // rowid alias and the kernel flagged it not-null.
            let cid_u16 = cid as u16;
            let is_rowid_alias = rowid_alias_ordinal == Some(cid_u16);
            let surface_not_null = if is_rowid_alias {
                explicit_not_null
                    .as_ref()
                    .is_some_and(|columns| columns.contains(&cid_u16))
            } else {
                column.not_null
            };
            let mut row = vec![
                SqlValue::Integer(cid as i64),
                SqlValue::Text(Arc::from(column.name.as_ref())),
                SqlValue::Text(Arc::from(column.declared_type.as_deref().unwrap_or(""))),
                SqlValue::Integer(if surface_not_null { 1 } else { 0 }),
                render_column_default(column),
                SqlValue::Integer(pk[cid]),
            ];
            if include_hidden {
                // Hidden ordinal: 0 = regular, 2 = generated VIRTUAL, 3 =
                // generated STORED. SQLite's xinfo uses these specific
                // numeric codes — see `sqlite_master.sql` documentation.
                let hidden = match column.generated.as_ref().map(|g| g.kind) {
                    None => 0,
                    Some(redlinedb_kernel::catalog::GeneratedColumnKind::Virtual) => 2,
                    Some(redlinedb_kernel::catalog::GeneratedColumnKind::Stored) => 3,
                };
                row.push(SqlValue::Integer(hidden));
            }
            row
        })
        .collect()
}

pub(crate) fn pragma_table_list_rows(
    conn: &Connection,
    schema: &SchemaSnapshot,
) -> Result<Vec<Vec<SqlValue>>> {
    let temp_tables = conn.with_session(|session| Ok(session.temp_tables.clone()))?;
    let mut rows = vec![table_list_row("main", "sqlite_schema", 5, false, false)];

    for table in &schema.tables {
        if temp_tables
            .iter()
            .any(|name| name.eq_ignore_ascii_case(table.name.as_ref()))
        {
            continue;
        }
        rows.push(table_list_row(
            "main",
            table.name.as_ref(),
            table.columns.len(),
            table.is_without_rowid(),
            table.is_strict(),
        ));
    }

    rows.push(table_list_row(
        concat!("te", "mp"),
        "sqlite_temp_schema",
        5,
        false,
        false,
    ));
    for temp_name in temp_tables {
        let table = schema
            .tables
            .iter()
            .find(|table| table.name.as_ref().eq_ignore_ascii_case(&temp_name));
        rows.push(match table {
            Some(table) => table_list_row(
                concat!("te", "mp"),
                table.name.as_ref(),
                table.columns.len(),
                table.is_without_rowid(),
                table.is_strict(),
            ),
            None => table_list_row(concat!("te", "mp"), temp_name.as_str(), 0, false, false),
        });
    }
    Ok(rows)
}

fn table_list_row(
    schema: &str,
    name: &str,
    ncol: usize,
    without_rowid: bool,
    strict: bool,
) -> Vec<SqlValue> {
    vec![
        SqlValue::Text(Arc::from(schema)),
        SqlValue::Text(Arc::from(name)),
        SqlValue::Text(Arc::from("table")),
        SqlValue::Integer(ncol as i64),
        SqlValue::Integer(if without_rowid { 1 } else { 0 }),
        SqlValue::Integer(if strict { 1 } else { 0 }),
    ]
}

pub(crate) fn pragma_index_list_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    // SQLite hides the implicit primary-key autoindex from
    // `PRAGMA index_list` when the table has a rowid-style INTEGER
    // PRIMARY KEY. The PK index in that case is the rowid itself; the
    // sqlite_autoindex_* entry is an internal artifact. We mirror that
    // surface so callers see only user-meaningful indexes.
    table
        .indexes
        .iter()
        .filter(|index| {
            !(index.primary
                && matches!(
                    index.origin,
                    redlinedb_kernel::catalog::IndexOrigin::PrimaryKey
                )
                && table.rowid_alias_column.is_some())
        })
        .rev()
        .enumerate()
        .map(|(seq, index)| {
            let origin = match index.origin {
                redlinedb_kernel::catalog::IndexOrigin::PrimaryKey => "pk",
                redlinedb_kernel::catalog::IndexOrigin::UniqueConstraint => "u",
                redlinedb_kernel::catalog::IndexOrigin::User => "c",
            };
            vec![
                SqlValue::Integer(seq as i64),
                SqlValue::Text(Arc::from(index.name.as_ref())),
                SqlValue::Integer(if index.unique { 1 } else { 0 }),
                SqlValue::Text(Arc::from(origin)),
                SqlValue::Integer(0),
            ]
        })
        .collect()
}

pub(crate) fn pragma_index_info_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = match schema.table_by_id(index.table_id) {
        Some(t) => t,
        None => {
            return Err(Error::UnsupportedSql(
                "index references missing table".to_owned(),
            ));
        }
    };
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
            continue;
        };
        let column = match table.columns.get(*attnum as usize) {
            Some(c) => c,
            None => {
                return Err(Error::UnsupportedSql(
                    "index references missing column".to_owned(),
                ));
            }
        };
        rows.push(vec![
            SqlValue::Integer(seqno as i64),
            SqlValue::Integer(*attnum as i64),
            SqlValue::Text(Arc::from(column.name.as_ref())),
        ]);
    }
    Ok(rows)
}

pub(crate) fn pragma_index_xinfo_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = match schema.table_by_id(index.table_id) {
        Some(t) => t,
        None => {
            return Err(Error::UnsupportedSql(
                "index references missing table".to_owned(),
            ));
        }
    };
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
            continue;
        };
        let column = match table.columns.get(*attnum as usize) {
            Some(c) => c,
            None => {
                return Err(Error::UnsupportedSql(
                    "index references missing column".to_owned(),
                ));
            }
        };
        rows.push(vec![
            SqlValue::Integer(seqno as i64),
            SqlValue::Integer(*attnum as i64),
            SqlValue::Text(Arc::from(column.name.as_ref())),
            SqlValue::Integer(
                if matches!(key.sort_dir, redlinedb_kernel::catalog::SortDir::Desc) {
                    1
                } else {
                    0
                },
            ),
            SqlValue::Text(Arc::from("BINARY")),
            SqlValue::Integer(1),
        ]);
    }
    Ok(rows)
}

/// Emit one row per pending deferred FK violation visible to the
/// current session. Uses the SessionState pointer installed by the
/// surrounding `with_write_tx` / `with_session` call to avoid
/// re-acquiring the connection mutex (which would deadlock against
/// the surrounding write transaction).
pub(crate) fn pragma_foreign_key_check_rows(
    conn: &Connection,
    schema: &SchemaSnapshot,
) -> Result<Vec<Vec<SqlValue>>> {
    crate::exec::fk::foreign_key_check_rows(conn, schema)
}

pub(crate) fn pragma_foreign_key_list_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    let mut rows = Vec::new();
    for (fk_id, fk) in table.foreign_keys.iter().enumerate() {
        let max_len = std::cmp::max(fk.columns.len(), fk.parent_columns.len());
        let parent_table_text: Arc<str> = Arc::from(fk.parent_table.as_ref());
        let on_update_text: Arc<str> = Arc::from(fk_action_to_str(fk.on_update));
        let on_delete_text: Arc<str> = Arc::from(fk_action_to_str(fk.on_delete));
        let match_text: Arc<str> = Arc::from("NONE");
        for seq in 0..max_len {
            let from_text = fk_from_column_name(table, fk, seq);
            let to_value = fk_parent_column_value(fk, seq);
            rows.push(vec![
                SqlValue::Integer(fk_id as i64),
                SqlValue::Integer(seq as i64),
                SqlValue::Text(Arc::clone(&parent_table_text)),
                SqlValue::Text(from_text),
                to_value,
                SqlValue::Text(Arc::clone(&on_update_text)),
                SqlValue::Text(Arc::clone(&on_delete_text)),
                SqlValue::Text(Arc::clone(&match_text)),
            ]);
        }
    }
    rows
}

/// Resolve the child-side column name at position `seq` to a typed
/// `Arc<str>`. SQLite always reports a child column (the FK either
/// names it explicitly or the binder folded it into `fk.columns`), so
/// missing-attnum and missing-column are kernel invariants violations
/// — we surface them as the literal `"?"` string rather than panicking
/// to keep the surface stable for diagnostics.
fn fk_from_column_name(
    table: &redlinedb_kernel::catalog::TableDef,
    fk: &redlinedb_kernel::catalog::ForeignKeyDef,
    seq: usize,
) -> Arc<str> {
    let Some(&attnum) = fk.columns.get(seq) else {
        return Arc::from("?");
    };
    let Some(column) = table.columns.get(attnum as usize) else {
        return Arc::from("?");
    };
    Arc::from(column.name.as_ref())
}

/// Render the parent-side column for one FK row. When the FK omits the
/// parent column list SQLite emits NULL (the executor resolves to the
/// parent table's PK at write time) so we mirror that surface.
fn fk_parent_column_value(fk: &redlinedb_kernel::catalog::ForeignKeyDef, seq: usize) -> SqlValue {
    if fk.parent_columns.is_empty() {
        return SqlValue::Null;
    }
    let Some(parent_col) = fk.parent_columns.get(seq) else {
        return SqlValue::Null;
    };
    SqlValue::Text(Arc::from(parent_col.as_ref()))
}

fn fk_action_to_str(action: redlinedb_kernel::catalog::FkAction) -> &'static str {
    use redlinedb_kernel::catalog::FkAction;
    match action {
        FkAction::NoAction => "NO ACTION",
        FkAction::Restrict => "RESTRICT",
        FkAction::Cascade => "CASCADE",
        FkAction::SetNull => "SET NULL",
        FkAction::SetDefault => "SET DEFAULT",
    }
}

/// `PRAGMA redline_index_check`: emit one row per catalog index reporting
/// validation status. Compatible with the previous engine-level
/// `integrity_check` API: empty `errors` list maps to "ok"; otherwise the
/// status column carries the comma-joined validation messages.
pub(crate) fn pragma_redline_index_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
    let result = conn.engine().integrity_check_per_index()?;
    Ok(result
        .into_iter()
        .map(|(name, errors)| {
            let status = if errors.is_empty() {
                Arc::<str>::from("ok")
            } else {
                Arc::<str>::from(errors.join(", ").as_str())
            };
            vec![
                SqlValue::Text(Arc::from(name.as_str())),
                SqlValue::Text(status),
            ]
        })
        .collect())
}

/// `PRAGMA redline_full_check`: run the full equivalence check and emit one
/// row per relation summarising heap/index counts plus aggregate page-level
/// counters. The `details` column carries any per-relation or top-level
/// error strings (semicolon-joined) so callers can surface both the
/// numeric mismatch and the underlying message in a single SELECT.
pub(crate) fn pragma_redline_full_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
    let report = conn.engine().integrity_check_full()?;
    let mut rows = Vec::with_capacity(report.relations.len());
    for relation in &report.relations {
        let entry_total: i64 = relation.indexes.iter().map(|i| i.entry_count as i64).sum();
        let heap_minus: i64 = relation
            .indexes
            .iter()
            .map(|i| i.heap_minus_index as i64)
            .sum();
        let index_minus: i64 = relation
            .indexes
            .iter()
            .map(|i| i.index_minus_heap as i64)
            .sum();
        let mut details: Vec<String> = relation.errors.clone();
        for ix in &relation.indexes {
            for err in &ix.structural_errors {
                details.push(format!("{}: {}", ix.index_name, err));
            }
            for err in &ix.errors {
                details.push(format!("{}: {}", ix.index_name, err));
            }
        }
        let status = if relation.errors.is_empty()
            && heap_minus == 0
            && index_minus == 0
            && relation
                .indexes
                .iter()
                .all(|i| i.structural_errors.is_empty() && i.errors.is_empty())
        {
            "ok"
        } else {
            "errors"
        };
        rows.push(vec![
            SqlValue::Text(Arc::from(relation.relation_name.as_str())),
            SqlValue::Text(Arc::from(status)),
            SqlValue::Integer(relation.heap_row_count as i64),
            SqlValue::Integer(entry_total),
            SqlValue::Integer(heap_minus),
            SqlValue::Integer(index_minus),
            SqlValue::Integer(report.page_csum_failures.len() as i64),
            SqlValue::Integer(report.lsn_monotonicity_violations.len() as i64),
            SqlValue::Text(Arc::from(details.join("; ").as_str())),
        ]);
    }
    if rows.is_empty() {
        // Surface aggregate page-level signals even when the schema has no
        // user tables yet, so callers can still consume a single deterministic
        // row with the page checksum / LSN violation counters.
        let status = if report.is_clean() { "ok" } else { "errors" };
        rows.push(vec![
            SqlValue::Text(Arc::from("(database)")),
            SqlValue::Text(Arc::from(status)),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(report.page_csum_failures.len() as i64),
            SqlValue::Integer(report.lsn_monotonicity_violations.len() as i64),
            SqlValue::Text(Arc::from(report.errors.join("; ").as_str())),
        ]);
    }
    Ok(rows)
}

fn render_column_default(column: &redlinedb_kernel::catalog::ColumnDef) -> SqlValue {
    let value = render_default_value(column.default_value.as_ref());
    if !matches!(value, SqlValue::Null) {
        return value;
    }
    render_default_expr(column.default_expr.as_deref())
}

fn render_default_expr(expr: Option<&redlinedb_kernel::catalog::CompiledExpr>) -> SqlValue {
    use redlinedb_kernel::catalog::ExprOp;

    let Some(expr) = expr else {
        return SqlValue::Null;
    };
    match expr.bytecode.as_ref() {
        [ExprOp::CurrentDate] => SqlValue::Text(Arc::from("CURRENT_DATE")),
        [ExprOp::CurrentTime] => SqlValue::Text(Arc::from("CURRENT_TIME")),
        [ExprOp::CurrentTimestamp] => SqlValue::Text(Arc::from("CURRENT_TIMESTAMP")),
        _ => SqlValue::Null,
    }
}

fn render_default_value(value: Option<&OwnedValue>) -> SqlValue {
    let Some(value) = value else {
        return SqlValue::Null;
    };
    match value {
        OwnedValue::Null => SqlValue::Null,
        OwnedValue::Integer(v) => SqlValue::Text(Arc::from(v.to_string())),
        OwnedValue::Real(v) => SqlValue::Text(Arc::from(v.to_string())),
        OwnedValue::Text(v) => {
            let escaped = v.replace('\'', "''");
            SqlValue::Text(Arc::from(format!("'{escaped}'")))
        }
        OwnedValue::Blob(v) => {
            use std::fmt::Write;

            let mut out = String::from("X'");
            for byte in v.iter() {
                write!(&mut out, "{byte:02X}").expect("write hex");
            }
            out.push('\'');
            SqlValue::Text(Arc::from(out))
        }
    }
}
