use super::*;

pub(crate) fn parse_pragma_template(
    conn: &Connection,
    sql: &str,
    lower: &str,
    schema_epoch: SchemaEpoch,
    schema: &SchemaSnapshot,
) -> Result<Option<PreparedTemplate>> {
    let body = sql.trim().trim_end_matches(';').trim();
    if !lower.starts_with("pragma") {
        return Ok(None);
    }
    let mut rest = body["pragma".len()..].trim_start();
    if rest.is_empty() {
        return Err(Error::UnsupportedSql("PRAGMA requires a name".to_owned()));
    }
    if let Some(after_main) = rest
        .strip_prefix("main.")
        .or_else(|| rest.strip_prefix("MAIN."))
    {
        rest = after_main.trim_start();
    }

    let (name, value) = split_pragma_name_value(rest)?;
    let name = name.to_ascii_lowercase();
    let template = match name.as_str() {
        "foreign_keys" => {
            if let Some(value) = value {
                let value = parse_pragma_bool(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetForeignKeys(value)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("foreign_keys")],
                    vec![vec![SqlValue::Integer(if conn.foreign_keys() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "user_version" => {
            if let Some(value) = value {
                let value = parse_pragma_integer(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetUserVersion(value)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("user_version")],
                    vec![vec![SqlValue::Integer(conn.user_version())]],
                )
            }
        }
        "schema_version" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA schema_version is read-only".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("schema_version")],
                vec![vec![SqlValue::Integer(schema_epoch.0 as i64)]],
            )
        }
        "database_list" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA database_list does not accept arguments".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seq"),
                    String::from("name"),
                    String::from("file"),
                ],
                vec![vec![
                    SqlValue::Integer(0),
                    SqlValue::Text(Arc::from("main")),
                    SqlValue::Text(Arc::from(conn.database_path().to_string_lossy().as_ref())),
                ]],
            )
        }
        "integrity_check" => {
            let rows = conn.integrity_check()?;
            let rows = if rows.is_empty() {
                vec![vec![SqlValue::Text(Arc::from("ok"))]]
            } else {
                rows.into_iter()
                    .map(|error| vec![SqlValue::Text(Arc::from(error))])
                    .collect()
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("integrity_check")],
                rows,
            )
        }
        "redline_index_check" => pragma_static_select(
            sql,
            schema_epoch,
            vec![String::from("index"), String::from("status")],
            pragma_redline_index_check_rows(conn)?,
        ),
        "redline_full_check" => pragma_static_select(
            sql,
            schema_epoch,
            vec![
                String::from("relation"),
                String::from("status"),
                String::from("heap_rows"),
                String::from("index_entries"),
                String::from("heap_minus_index"),
                String::from("index_minus_heap"),
                String::from("page_csum_failures"),
                String::from("lsn_violations"),
                String::from("details"),
            ],
            pragma_redline_full_check_rows(conn)?,
        ),
        "table_info" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA table_info requires a table".to_owned())
            })?;
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("cid"),
                    String::from("name"),
                    String::from("type"),
                    String::from("notnull"),
                    String::from("dflt_value"),
                    String::from("pk"),
                ],
                pragma_table_info_rows(&table),
            )
        }
        "table_xinfo" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA table_xinfo requires a table".to_owned())
            })?;
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("cid"),
                    String::from("name"),
                    String::from("type"),
                    String::from("notnull"),
                    String::from("dflt_value"),
                    String::from("pk"),
                    String::from("hidden"),
                ],
                pragma_table_xinfo_rows(&table),
            )
        }
        "table_list" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA table_list does not accept arguments".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("schema"),
                    String::from("name"),
                    String::from("type"),
                    String::from("ncol"),
                    String::from("wr"),
                    String::from("strict"),
                ],
                pragma_table_list_rows(schema),
            )
        }
        "index_list" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA index_list requires a table".to_owned())
            })?;
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seq"),
                    String::from("name"),
                    String::from("unique"),
                    String::from("origin"),
                    String::from("partial"),
                ],
                pragma_index_list_rows(&table),
            )
        }
        "index_info" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA index_info requires an index".to_owned())
            })?;
            let index_name = parse_pragma_object_name(&name)?;
            let index = lookup_index(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(index_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seqno"),
                    String::from("cid"),
                    String::from("name"),
                ],
                pragma_index_info_rows(schema, &index)?,
            )
        }
        "index_xinfo" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA index_xinfo requires an index".to_owned())
            })?;
            let index_name = parse_pragma_object_name(&name)?;
            let index = lookup_index(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(index_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seqno"),
                    String::from("cid"),
                    String::from("name"),
                    String::from("desc"),
                    String::from("coll"),
                    String::from("key"),
                ],
                pragma_index_xinfo_rows(schema, &index)?,
            )
        }
        "foreign_key_list" => {
            let name = value.ok_or_else(|| {
                Error::UnsupportedSql("PRAGMA foreign_key_list requires a table".to_owned())
            })?;
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("id"),
                    String::from("seq"),
                    String::from("table"),
                    String::from("from"),
                    String::from("to"),
                    String::from("on_update"),
                    String::from("on_delete"),
                    String::from("match"),
                    String::from("deferred"),
                ],
                pragma_foreign_key_list_rows(&table),
            )
        }
        _ => return Ok(None),
    };
    Ok(Some(template))
}

fn pragma_static_select(
    sql: &str,
    schema_epoch: SchemaEpoch,
    output_columns: impl Into<Arc<[String]>>,
    rows: Vec<Vec<SqlValue>>,
) -> PreparedTemplate {
    PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source: SelectSource::StaticRows {
                rows: Arc::from(rows),
            },
            distinct: false,
            projection: Vec::new(),
            selection: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }),
    }
}

fn split_pragma_name_value(input: &str) -> Result<(String, Option<String>)> {
    let trimmed = input.trim();
    if let Some((name, value)) = trimmed.split_once('=') {
        return Ok((
            name.trim().to_owned(),
            Some(value.trim().trim_end_matches(';').trim().to_owned()),
        ));
    }
    if let Some(start) = trimmed.find('(') {
        let end = trimmed
            .rfind(')')
            .ok_or_else(|| Error::UnsupportedSql("unterminated PRAGMA argument".to_owned()))?;
        let name = trimmed[..start].trim().to_owned();
        let value = trimmed[start + 1..end].trim();
        return Ok((
            name,
            if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            },
        ));
    }
    Ok((trimmed.to_owned(), None))
}

fn parse_pragma_bool(input: &str) -> Result<bool> {
    let value = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match value.to_ascii_lowercase().as_str() {
        "1" | "on" | "true" => Ok(true),
        "0" | "off" | "false" => Ok(false),
        other => Err(Error::UnsupportedSql(format!(
            "invalid boolean PRAGMA value: {other}"
        ))),
    }
}

fn parse_pragma_integer(input: &str) -> Result<i64> {
    let value = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    value
        .parse::<i64>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid integer PRAGMA value: {value}")))
}

fn parse_pragma_object_name(input: &str) -> Result<String> {
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    let mut parts = token.splitn(2, '.');
    let first = parts.next().unwrap_or_default();
    if let Some(second) = parts.next() {
        if first.eq_ignore_ascii_case("main") {
            return Ok(second.to_owned());
        }
        return Err(Error::UnsupportedSql(format!(
            "unsupported PRAGMA object qualifier: {first}"
        )));
    }
    Ok(token)
}

fn unquote_pragma_token(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let bytes = trimmed.as_bytes();
    match (bytes.first().copied()?, bytes.last().copied()?) {
        (b'\'', b'\'') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("''", "'"))
        }
        (b'"', b'"') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("\"\"", "\""))
        }
        (b'`', b'`') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("``", "`"))
        }
        (b'[', b']') if trimmed.len() >= 2 => Some(trimmed[1..trimmed.len() - 1].to_owned()),
        _ => None,
    }
}

fn pragma_table_info_rows(table: &redlinedb_kernel::catalog::TableDef) -> Vec<Vec<SqlValue>> {
    pragma_column_rows(table, false)
}

fn pragma_table_xinfo_rows(table: &redlinedb_kernel::catalog::TableDef) -> Vec<Vec<SqlValue>> {
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
            let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source;
            if let Some(slot) = pk.get_mut(*attnum as usize) {
                *slot = (position + 1) as i64;
            }
        }
    }

    table
        .columns
        .iter()
        .enumerate()
        .map(|(cid, column)| {
            let mut row = vec![
                SqlValue::Integer(cid as i64),
                SqlValue::Text(Arc::from(column.name.as_ref())),
                SqlValue::Text(Arc::from(column.declared_type.as_deref().unwrap_or(""))),
                SqlValue::Integer(if column.not_null { 1 } else { 0 }),
                render_default_value(column.default_value.as_ref()),
                SqlValue::Integer(pk[cid]),
            ];
            if include_hidden {
                row.push(SqlValue::Integer(0));
            }
            row
        })
        .collect()
}

fn pragma_table_list_rows(schema: &SchemaSnapshot) -> Vec<Vec<SqlValue>> {
    schema
        .tables
        .iter()
        .map(|table| {
            vec![
                SqlValue::Text(Arc::from("main")),
                SqlValue::Text(Arc::from(table.name.as_ref())),
                SqlValue::Text(Arc::from("table")),
                SqlValue::Integer(table.columns.len() as i64),
                SqlValue::Integer(0),
                SqlValue::Integer(if table.flags != 0 { 1 } else { 0 }),
            ]
        })
        .collect()
}

fn pragma_index_list_rows(table: &redlinedb_kernel::catalog::TableDef) -> Vec<Vec<SqlValue>> {
    table
        .indexes
        .iter()
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

fn pragma_index_info_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = schema
        .table_by_id(index.table_id)
        .ok_or_else(|| Error::UnsupportedSql("index references missing table".to_owned()))?;
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source;
        let column = table
            .columns
            .get(*attnum as usize)
            .ok_or_else(|| Error::UnsupportedSql("index references missing column".to_owned()))?;
        rows.push(vec![
            SqlValue::Integer(seqno as i64),
            SqlValue::Integer(*attnum as i64),
            SqlValue::Text(Arc::from(column.name.as_ref())),
        ]);
    }
    Ok(rows)
}

fn pragma_index_xinfo_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = schema
        .table_by_id(index.table_id)
        .ok_or_else(|| Error::UnsupportedSql("index references missing table".to_owned()))?;
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source;
        let column = table
            .columns
            .get(*attnum as usize)
            .ok_or_else(|| Error::UnsupportedSql("index references missing column".to_owned()))?;
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

fn pragma_foreign_key_list_rows(
    _table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    Vec::new()
}

/// `PRAGMA redline_index_check`: emit one row per catalog index reporting
/// validation status. Compatible with the previous engine-level
/// `integrity_check` API: empty `errors` list maps to "ok"; otherwise the
/// status column carries the comma-joined validation messages.
fn pragma_redline_index_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
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
fn pragma_redline_full_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
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
