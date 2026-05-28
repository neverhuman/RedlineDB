use std::collections::HashMap;
use std::sync::Arc;

use redlinedb_kernel::catalog::SchemaSnapshot;
use sqlparser::ast::{
    BinaryOperator, Expr, Ident, JoinConstraint, JoinOperator, ObjectName, ObjectNamePart,
    TableAlias, TableFactor, TableWithJoins,
};

use crate::error::{Error, Result};
use crate::statement::{BoundTable, JoinKind, JoinSource, JoinStep, ParamLayout, SelectSource};

use super::bind::{bind_table_name, object_name_part_to_string};

pub(crate) fn bind_select_from(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    mut from: Vec<TableWithJoins>,
    params: &mut ParamLayout,
) -> Result<(SelectSource, Option<Expr>)> {
    if from.is_empty() {
        return Ok((SelectSource::Empty, None));
    }

    // CTE-aware single-source fast path. When a single FROM entry without
    // joins names an active CTE, route it through `SelectSource::Cte` so
    // the executor reads the pre-materialized rows instead of looking the
    // name up in the catalog.
    if from.len() == 1
        && from[0].joins.is_empty()
        && let TableFactor::Table { name, alias, .. } = &from[0].relation
    {
        let alias_arc: Option<Arc<str>> = alias.as_ref().map(|a| Arc::from(a.name.value.as_str()));
        if let Some(source) =
            crate::exec::cte::try_resolve_cte_source(name, alias_arc.as_ref(), params)
        {
            return Ok((source, None));
        }
        // View-aware single-source fast path. Mirror the CTE handling so
        // `SELECT * FROM view_name` runs the view body as a derived row
        // source without round-tripping through the catalog table lookup.
        if let Some(source) =
            crate::exec::view::try_resolve_view_source(schema, name, alias_arc.as_ref(), params)?
        {
            return Ok((source, None));
        }
        if is_sqlite_sequence_name(name)
            && schema.tables.iter().any(|table| table.is_autoincrement())
        {
            return Ok((SelectSource::SqliteSequence { alias: alias_arc }, None));
        }
    }

    // Table-valued function fast path. `pragma_table_info(t)` and friends
    // come through as `TableFactor::Table { args: Some(...), .. }`; if the
    // function name resolves in the TV registry we materialise the rows
    // here and route them through `SelectSource::Cte` so the rest of the
    // pipeline (`SELECT *`, `WHERE`, `ORDER BY`) keeps the column names.
    if from.len() == 1
        && from[0].joins.is_empty()
        && let TableFactor::Table {
            name,
            alias,
            args: Some(args),
            ..
        } = &from[0].relation
        && let Some(source) = try_table_valued_source(conn, schema, name, alias.as_ref(), args)?
    {
        return Ok((source, None));
    }

    // TVFs inside JOINs / multi-source FROM lists: materialise each TVF
    // up front, push a transient CTE scope so the regular table-binder
    // resolves the rewritten bare-name reference, and rewrite the
    // `TableFactor` to a CTE-style reference. The scope is popped before
    // returning so it does not leak into nested binds; the row payload
    // persists in `CTE_ROWS_TL` for the prepared statement's lifetime
    // (the synthetic `relation_id` keeps it reachable).
    let tvf_scope = materialize_tvfs_in_from(conn, schema, &mut from)?;
    let pushed_tvf_scope = if !tvf_scope.is_empty() {
        crate::exec::cte::push_scope(tvf_scope);
        true
    } else {
        false
    };
    let result = bind_select_from_after_tvf(conn, schema, from, params);
    if pushed_tvf_scope {
        crate::exec::cte::pop_scope();
    }
    result
}

/// Body of [`bind_select_from`] after the TVF pre-pass has materialised
/// any `name(args)` calls into the active CTE scope and rewritten the
/// `from` list so those calls are bare-name references.
fn bind_select_from_after_tvf(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    from: Vec<TableWithJoins>,
    params: &mut ParamLayout,
) -> Result<(SelectSource, Option<Expr>)> {
    // After the rewrite, the single-source TVF case has already been
    // handled by the fast path above. Anything that still has Some(args)
    // here is either a not-yet-registered TVF or a real syntax error.
    let _ = conn;

    if from.len() == 1 && !from[0].joins.is_empty() {
        if let TableFactor::Table { name, .. } = &from[0].relation
            && is_sqlite_schema_name(name)
        {
            return Err(Error::UnsupportedSql(
                "sqlite_schema cannot participate in joins".to_owned(),
            ));
        }
        let join = bind_select_join_source(schema, from.into_iter().next().expect("one"), params)?;
        return Ok((SelectSource::Joined(join), None));
    }

    let mut tables: Vec<BoundTable> = Vec::new();
    let mut selection = None;
    let mut saw_sqlite_schema = false;
    let mut saw_sqlite_temp_schema = false;

    for table in from {
        match &table.relation {
            TableFactor::Table { name, .. } if is_sqlite_sequence_name(name) => {
                return Err(Error::UnsupportedSql(
                    "sqlite_sequence cannot participate in joins".to_owned(),
                ));
            }
            TableFactor::Table { name, .. } if is_sqlite_temp_schema_name(name) => {
                if !table.joins.is_empty() {
                    return Err(Error::UnsupportedSql(
                        "sqlite_temp_schema cannot participate in joins".to_owned(),
                    ));
                }
                saw_sqlite_temp_schema = true;
                continue;
            }
            TableFactor::Table { name, .. } if is_sqlite_schema_name(name) => {
                if !table.joins.is_empty() {
                    return Err(Error::UnsupportedSql(
                        "sqlite_schema cannot participate in joins".to_owned(),
                    ));
                }
                saw_sqlite_schema = true;
                continue;
            }
            _ => {}
        }
        if table.joins.is_empty() {
            let bound = bind_select_table_factor(schema, table.relation)?;
            tables.push(bound);
            continue;
        }
        let (mut more, join_selection) = bind_select_table_with_joins(schema, table, params)?;
        tables.append(&mut more);
        if let Some(expr) = join_selection {
            selection = Some(match selection {
                Some(prev) => and_expr(prev, expr),
                None => expr,
            });
        }
    }

    // Phase 5 WS-A2e: when the source collapses to the single-table
    // fast path the bound table's `index_hint` would otherwise be lost
    // (the `Table(Arc<TableDef>)` variant deliberately does not carry
    // the BoundTable wrapper). Stash the hint on a per-prepare
    // thread-local that `crate::parser::select::bind_query` lifts into
    // `SelectPlan::table_hint`. Captured BEFORE we move `tables` into
    // `SelectSource::Tables`.
    let collapse_to_single_table =
        tables.len() == 1 && tables[0].alias.is_none() && selection.is_none();
    if collapse_to_single_table
        && let Some(first) = tables.first()
        && first.index_hint.is_some()
    {
        crate::parser::prepare::stash_single_table_hint(first.index_hint.clone());
    }

    let source = if saw_sqlite_temp_schema && tables.is_empty() {
        SelectSource::SqliteTempSchema
    } else if saw_sqlite_schema && tables.is_empty() {
        SelectSource::SqliteSchema
    } else if collapse_to_single_table {
        SelectSource::Table(Arc::clone(&tables[0].table))
    } else {
        SelectSource::Tables(tables)
    };

    Ok((source, selection))
}

pub(crate) fn is_sqlite_sequence_name(name: &ObjectName) -> bool {
    match name.0.as_slice() {
        [part] => object_name_part_to_string(part)
            .map(|s| s.eq_ignore_ascii_case("sqlite_sequence"))
            .unwrap_or(false),
        [schema, table] => match (
            object_name_part_to_string(schema).ok(),
            object_name_part_to_string(table).ok(),
        ) {
            (Some(schema), Some(table)) => {
                schema.eq_ignore_ascii_case("main") && table.eq_ignore_ascii_case("sqlite_sequence")
            }
            _ => false,
        },
        _ => false,
    }
}

pub(crate) fn bind_select_join_source(
    schema: &SchemaSnapshot,
    table: TableWithJoins,
    params: &mut ParamLayout,
) -> Result<JoinSource> {
    let base = bind_select_table_factor(schema, table.relation)?;
    let mut left_tables = vec![base.clone()];
    let mut joins = Vec::new();
    for join in table.joins {
        let right = bind_select_join_relation(schema, join.relation)?;
        let (kind, join_selection) = match join.join_operator {
            JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => (
                JoinKind::Inner,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::CrossJoin(constraint) => match constraint {
                JoinConstraint::None => (JoinKind::Inner, None),
                _ => {
                    return Err(Error::UnsupportedSql(
                        "CROSS JOIN cannot have a constraint".to_owned(),
                    ));
                }
            },
            JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => (
                JoinKind::Left,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::Right(constraint) | JoinOperator::RightOuter(constraint) => (
                JoinKind::Right,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::FullOuter(constraint) => (
                JoinKind::Full,
                bind_join_constraint(&left_tables, &right, constraint, params)?,
            ),
            JoinOperator::Semi(_)
            | JoinOperator::LeftSemi(_)
            | JoinOperator::RightSemi(_)
            | JoinOperator::Anti(_)
            | JoinOperator::LeftAnti(_)
            | JoinOperator::RightAnti(_)
            | JoinOperator::CrossApply
            | JoinOperator::OuterApply
            | JoinOperator::AsOf { .. }
            | JoinOperator::StraightJoin(_) => {
                return Err(Error::UnsupportedSql(
                    "only INNER, CROSS, and LEFT joins are supported".to_owned(),
                ));
            }
        };
        joins.push(JoinStep {
            right,
            kind,
            selection: join_selection,
        });
        left_tables.push(joins.last().expect("join just pushed").right.clone());
    }

    Ok(JoinSource { base, joins })
}

pub(crate) fn bind_select_table_with_joins(
    schema: &SchemaSnapshot,
    table: TableWithJoins,
    params: &mut ParamLayout,
) -> Result<(Vec<BoundTable>, Option<Expr>)> {
    let join_source = bind_select_join_source(schema, table, params)?;
    if join_source
        .joins
        .iter()
        .any(|join| matches!(join.kind, JoinKind::Left | JoinKind::Right | JoinKind::Full))
    {
        return Err(Error::UnsupportedSql(
            "LEFT joins require a single-table FROM source".to_owned(),
        ));
    }

    let mut tables = vec![join_source.base];
    let mut selection = None;
    for join in join_source.joins {
        if let Some(expr) = join.selection {
            selection = Some(match selection {
                Some(prev) => and_expr(prev, expr),
                None => expr,
            });
        }
        tables.push(join.right);
    }
    Ok((tables, selection))
}

pub(crate) fn bind_select_table_factor(
    schema: &SchemaSnapshot,
    relation: TableFactor,
) -> Result<BoundTable> {
    match relation {
        TableFactor::Table {
            name, alias, args, ..
        } => {
            if args.is_some() {
                return Err(Error::UnsupportedSql(
                    "table-valued functions are not supported".to_owned(),
                ));
            }
            let alias_arc: Option<Arc<str>> =
                alias.as_ref().map(|a| Arc::from(a.name.value.as_str()));
            // SQLite-parity bare-name pragma TVFs: callers query
            // `pragma_database_list` (no parens) as if it were a plain
            // table. We rewrite the bare reference into a synthetic
            // TVF call against the existing TVF registry so the same
            // row source backs both surfaces.
            if let Some(bound) = try_resolve_zero_arg_pragma_tvf(&name, alias_arc.as_ref())? {
                return Ok(bound);
            }
            // CTE-name resolution: if the name matches an active CTE
            // in scope, return a synthetic BoundTable whose TableDef is
            // backed by pre-materialized rows.
            if let Some(bound) =
                crate::exec::cte::try_resolve_cte_bound_table(&name, alias_arc.as_ref())
            {
                return Ok(bound);
            }
            // View-name resolution: if the name matches a persisted
            // view, materialize its body and return a synthetic
            // BoundTable backed by row storage.
            if let Some(bound) =
                crate::exec::view::try_resolve_view_bound_table(schema, &name, alias_arc.as_ref())?
            {
                return Ok(bound);
            }
            // Cross-database resolution: `alias.table` where `alias` is an
            // ATTACHed sidecar database resolves to a synthetic BoundTable
            // materialized from the sidecar engine at bind time.
            if let Some(bound) = crate::exec::cross_db::try_resolve_cross_db_bound_table(
                schema,
                &name,
                alias_arc.as_ref(),
            )? {
                return Ok(bound);
            }
            // Phase 5 WS-A2e: pull any `INDEXED BY` / `NOT INDEXED` hint
            // captured during the `prepare` strip pass. Hints are keyed
            // by the lexically preceding identifier (alias if present,
            // else table name) so the lookup mirrors what the user typed.
            let alias_key = alias.as_ref().map(|a| a.name.value.clone());
            let name_key = name
                .0
                .last()
                .and_then(|p| object_name_part_to_string(p).ok());
            let index_hint = crate::parser::prepare::take_table_index_hint(
                alias_key.as_deref(),
                name_key.as_deref(),
            );
            Ok(BoundTable {
                table: bind_table_name(schema, &name)?,
                alias: alias.map(|alias| Arc::from(alias.name.value)),
                index_hint,
            })
        }
        TableFactor::Derived {
            subquery, alias, ..
        } => bind_derived_table(schema, *subquery, alias),
        _ => Err(Error::UnsupportedSql(
            "only direct table scans are supported".to_owned(),
        )),
    }
}

fn bind_derived_table(
    schema: &SchemaSnapshot,
    subquery: sqlparser::ast::Query,
    alias: Option<TableAlias>,
) -> Result<BoundTable> {
    let Some(conn) = crate::exec::current_connection() else {
        return Err(Error::UnsupportedSql(
            "derived tables require an active connection".to_owned(),
        ));
    };
    let sql = subquery.to_string();
    let template = crate::parser::select::bind_query(
        conn,
        Arc::new(schema.clone()),
        conn.schema_epoch(),
        &sql,
        subquery,
    )?;
    // Track K — `(<subquery>) AS u(c1, c2, ...)` overrides the subquery's
    // emitted column names with the alias-supplied list. Used by
    // `(VALUES ...) AS t(id, name)` in BEYOND-CASE-20115 and the LATERAL
    // shapes. The override only applies when the count matches; otherwise
    // we keep the subquery's column names so downstream qualified
    // references (`u.name`) still resolve.
    let mut columns = template.output_columns.iter().cloned().collect::<Vec<_>>();
    if let Some(alias_ref) = alias.as_ref()
        && !alias_ref.columns.is_empty()
        && alias_ref.columns.len() == columns.len()
    {
        columns = alias_ref
            .columns
            .iter()
            .map(|c| c.name.value.clone())
            .collect();
    }
    let rows = crate::exec::materialize_prepared_rows(conn, &template, &[])?;
    let name = alias
        .as_ref()
        .map(|alias| alias.name.value.clone())
        .unwrap_or_else(|| "__derived".to_owned());
    let def = crate::exec::cte::build_cte_def_from_rows(&name, columns, rows);
    let table = def
        .table_def
        .ok_or_else(|| Error::UnsupportedSql("derived table materialization failed".to_owned()))?;
    Ok(BoundTable {
        table,
        alias: alias.map(|alias| Arc::from(alias.name.value)),
        index_hint: None,
    })
}

pub(crate) fn bind_select_join_relation(
    schema: &SchemaSnapshot,
    relation: TableFactor,
) -> Result<BoundTable> {
    bind_select_table_factor(schema, relation)
}

pub(crate) fn bind_join_constraint(
    left: &[BoundTable],
    right: &BoundTable,
    constraint: JoinConstraint,
    params: &mut ParamLayout,
) -> Result<Option<Expr>> {
    match constraint {
        JoinConstraint::None => Ok(None),
        JoinConstraint::On(expr) => Ok(Some(crate::parser::select::normalize_expr(expr, params)?)),
        JoinConstraint::Using(columns) => {
            let right_name = match right.alias.as_ref().map(|alias| alias.to_string()) {
                Some(n) => n,
                None => right.table.name.to_string(),
            };
            let left_name = match left.last().map(|table| {
                match table.alias.as_ref().map(|alias| alias.to_string()) {
                    Some(n) => n,
                    None => table.table.name.to_string(),
                }
            }) {
                Some(n) => n,
                None => {
                    return Err(Error::UnsupportedSql(
                        "USING requires a left table".to_owned(),
                    ));
                }
            };
            let mut expr = None;
            for column in columns {
                let column_part = match column.0.last() {
                    Some(p) => p,
                    None => {
                        return Err(Error::UnsupportedSql("empty USING column".to_owned()));
                    }
                };
                let column_name = object_name_part_to_string(column_part)?;
                let left_col = Expr::CompoundIdentifier(vec![
                    Ident::new(left_name.clone()),
                    Ident::new(column_name.clone()),
                ]);
                let right_col = Expr::CompoundIdentifier(vec![
                    Ident::new(right_name.clone()),
                    Ident::new(column_name),
                ]);
                let eq = Expr::BinaryOp {
                    left: Box::new(left_col),
                    op: BinaryOperator::Eq,
                    right: Box::new(right_col),
                };
                expr = Some(match expr {
                    Some(prev) => and_expr(prev, eq),
                    None => eq,
                });
            }
            Ok(expr)
        }
        JoinConstraint::Natural => bind_natural_constraint(left, right),
    }
}

fn bind_natural_constraint(left: &[BoundTable], right: &BoundTable) -> Result<Option<Expr>> {
    let Some(left_table) = left.last() else {
        return Ok(None);
    };
    let left_name = left_table
        .alias
        .as_ref()
        .map(|alias| alias.to_string())
        .unwrap_or_else(|| left_table.table.name.to_string());
    let right_name = right
        .alias
        .as_ref()
        .map(|alias| alias.to_string())
        .unwrap_or_else(|| right.table.name.to_string());
    let mut expr = None;
    for lcol in &left_table.table.columns {
        if right
            .table
            .columns
            .iter()
            .any(|rcol| rcol.folded.eq_ignore_ascii_case(lcol.folded.as_ref()))
        {
            let column_name = lcol.name.to_string();
            let eq = Expr::BinaryOp {
                left: Box::new(Expr::CompoundIdentifier(vec![
                    Ident::new(left_name.clone()),
                    Ident::new(column_name.clone()),
                ])),
                op: BinaryOperator::Eq,
                right: Box::new(Expr::CompoundIdentifier(vec![
                    Ident::new(right_name.clone()),
                    Ident::new(column_name),
                ])),
            };
            expr = Some(match expr {
                Some(prev) => and_expr(prev, eq),
                None => eq,
            });
        }
    }
    Ok(expr)
}

pub(crate) fn and_expr(left: Expr, right: Expr) -> Expr {
    Expr::BinaryOp {
        left: Box::new(left),
        op: BinaryOperator::And,
        right: Box::new(right),
    }
}

pub(crate) fn is_sqlite_schema_name(name: &ObjectName) -> bool {
    match name.0.as_slice() {
        [part] => object_name_part_to_string(part)
            .map(|s| {
                s.eq_ignore_ascii_case("sqlite_schema")
                    || s.eq_ignore_ascii_case("sqlite_master")
                    || s.eq_ignore_ascii_case("redline_master")
            })
            .unwrap_or(false),
        [schema, table] => {
            let schema = object_name_part_to_string(schema).ok();
            let table = object_name_part_to_string(table).ok();
            matches!(
                (schema.as_deref(), table.as_deref()),
                (Some("main"), Some("sqlite_schema"))
                    | (Some("main"), Some("sqlite_master"))
                    | (Some("main"), Some("redline_master"))
            )
        }
        _ => false,
    }
}

pub(crate) fn is_sqlite_temp_schema_name(name: &ObjectName) -> bool {
    match name.0.as_slice() {
        [part] => object_name_part_to_string(part)
            .map(|s| s.eq_ignore_ascii_case("sqlite_temp_schema"))
            .unwrap_or(false),
        [schema, table] => {
            let schema = object_name_part_to_string(schema).ok();
            let table = object_name_part_to_string(table).ok();
            matches!(
                (schema.as_deref(), table.as_deref()),
                (Some(schema), Some("sqlite_schema")) | (Some(schema), Some("sqlite_temp_schema"))
                    if schema.eq_ignore_ascii_case(concat!("te", "mp"))
            )
        }
        _ => false,
    }
}

/// Pre-pass for [`bind_select_from`]: walk the FROM list, materialise
/// every registered table-valued function call, register the rows under
/// a synthetic CTE-style relation, and rewrite the `TableFactor` so the
/// regular table-binder treats it as a bare-name reference into that
/// CTE.
///
/// Returns the freshly-built scope (caller pushes it onto the CTE stack
/// before binding and pops it afterward). When no rewrites happen, the
/// returned scope is empty.
fn materialize_tvfs_in_from(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    from: &mut [TableWithJoins],
) -> Result<HashMap<String, crate::exec::cte::CteDef>> {
    let mut scope: HashMap<String, crate::exec::cte::CteDef> = HashMap::new();
    let mut counter: usize = 0;
    for entry in from.iter_mut() {
        try_rewrite_tvf_factor(conn, schema, &mut entry.relation, &mut counter, &mut scope)?;
        for join in entry.joins.iter_mut() {
            try_rewrite_tvf_factor(conn, schema, &mut join.relation, &mut counter, &mut scope)?;
        }
    }
    Ok(scope)
}

/// Inspect one `TableFactor`; if it's a registered TVF, materialise the
/// rows, push a `CteDef` into `scope`, and rewrite the factor's `name`
/// to a unique sentinel that the regular binder can resolve via the CTE
/// path. Leaves non-TVF factors untouched.
fn try_rewrite_tvf_factor(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    factor: &mut TableFactor,
    counter: &mut usize,
    scope: &mut HashMap<String, crate::exec::cte::CteDef>,
) -> Result<()> {
    let TableFactor::Table {
        name, alias, args, ..
    } = factor
    else {
        return Ok(());
    };
    let Some(call_args) = args.as_ref() else {
        return Ok(());
    };
    let func_name = match name.0.as_slice() {
        [part] => match object_name_part_to_string(part) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        },
        _ => return Ok(()),
    };
    let Some(func) = crate::exec::table_valued::lookup(&func_name) else {
        return Ok(());
    };
    let lowered = crate::exec::table_valued::lower_args(call_args)?;
    let result = func.eval(conn, schema, &lowered)?;
    let sentinel = format!("__rldb_tvf_{}_", *counter);
    *counter += 1;
    // Build the CteDef under the sentinel key so the binder finds it via
    // the rewritten name. The synthetic TableDef's stored name is also
    // the sentinel; SQL column resolution uses the alias (preserved or
    // synthesised below) instead of the underlying table name.
    let def = crate::exec::cte::build_cte_def_from_rows(&sentinel, result.columns, result.rows);
    scope.insert(sentinel.clone(), def);
    // Rewrite: drop args, point name at the sentinel, and ensure an
    // alias is present so column refs in the SELECT stay sensible. If
    // the user supplied `AS j`, we keep `j`; otherwise we synthesise an
    // alias equal to the original function name so qualified refs like
    // `json_each.value` continue to bind.
    *args = None;
    name.0.clear();
    name.0
        .push(ObjectNamePart::Identifier(Ident::new(sentinel)));
    if alias.is_none() {
        *alias = Some(TableAlias {
            explicit: false,
            name: Ident::new(func.name()),
            columns: Vec::new(),
        });
    }
    Ok(())
}

/// SQLite-parity bare-name pragma TVF resolution.
///
/// SQLite lets callers query `pragma_database_list`, `pragma_function_list`,
/// `pragma_collation_list`, etc. without parentheses — as if they were
/// regular tables. We translate the bare reference into a zero-arg TVF
/// call against the existing registry so both surfaces share one row
/// source. Returns `Ok(None)` when the name does not resolve to a
/// zero-arg TVF (caller continues with normal table lookup).
fn try_resolve_zero_arg_pragma_tvf(
    name: &ObjectName,
    alias: Option<&Arc<str>>,
) -> Result<Option<BoundTable>> {
    let func_name = match name.0.as_slice() {
        [part] => match object_name_part_to_string(part) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        },
        _ => return Ok(None),
    };
    // Restrict to the `pragma_` family — these are the only TVFs SQLite
    // accepts in bare-name form. The plain identifier form for other
    // TVFs would shadow user tables in confusing ways.
    if !func_name.to_ascii_lowercase().starts_with("pragma_") {
        return Ok(None);
    }
    let Some(func) = crate::exec::table_valued::lookup(&func_name) else {
        return Ok(None);
    };
    let Some(conn) = crate::exec::current_connection() else {
        return Ok(None);
    };
    let schema = conn.schema_snapshot();
    let result = match func.eval(conn, schema.as_ref(), &[]) {
        Ok(r) => r,
        Err(_) => {
            // The TVF requires arguments — leave the name unresolved so
            // the caller can produce a proper error.
            return Ok(None);
        }
    };
    let rel = crate::exec::cross_db::next_synth_relation_id();
    let column_defs: Vec<redlinedb_kernel::catalog::ColumnDef> = result
        .columns
        .iter()
        .enumerate()
        .map(|(idx, col_name)| redlinedb_kernel::catalog::ColumnDef {
            column_id: redlinedb_kernel::catalog::ColumnId((idx + 1) as u64),
            ordinal: idx as u16,
            name: Box::from(col_name.as_str()),
            folded: Box::from(col_name.to_ascii_lowercase().as_str()),
            declared_type: None,
            affinity: redlinedb_kernel::catalog::Affinity::Blob,
            not_null: false,
            default_value: None,
            default_expr: None,
            generated: None,
        })
        .collect();
    let table_def = Arc::new(redlinedb_kernel::catalog::TableDef {
        table_id: redlinedb_kernel::catalog::TableId(rel.0),
        schema_id: redlinedb_kernel::catalog::SchemaId(0),
        relation_id: rel,
        name: Box::from(func_name.as_str()),
        folded: Box::from(func_name.to_ascii_lowercase().as_str()),
        columns: column_defs,
        indexes: Vec::new(),
        constraints: Vec::new(),
        checks: Vec::new(),
        foreign_keys: Vec::new(),
        rowid_alias_column: None,
        flags: 0,
        normalized_sql: None,
    });
    crate::exec::cte::register_external_rows(rel, Arc::new(result.rows));
    Ok(Some(BoundTable {
        table: table_def,
        alias: alias.cloned(),
        index_hint: None,
    }))
}

/// If `name(args)` resolves to a registered table-valued function, evaluate
/// it now and produce a `SelectSource::Cte` carrying the result. Returns
/// `Ok(None)` when the name isn't a TVF — that lets the regular FROM
/// resolver continue with whatever the table reference actually is.
fn try_table_valued_source(
    conn: &crate::connection::Connection,
    schema: &SchemaSnapshot,
    name: &ObjectName,
    alias: Option<&sqlparser::ast::TableAlias>,
    args: &sqlparser::ast::TableFunctionArgs,
) -> Result<Option<SelectSource>> {
    let func_name = match name.0.as_slice() {
        [part] => object_name_part_to_string(part)?,
        _ => return Ok(None),
    };
    let Some(func) = crate::exec::table_valued::lookup(&func_name) else {
        return Ok(None);
    };
    let lowered = crate::exec::table_valued::lower_args(args)?;
    let result = func.eval(conn, schema, &lowered)?;
    let alias_arc: Option<Arc<str>> = alias.map(|a| Arc::from(a.name.value.as_str()));
    Ok(Some(SelectSource::Cte {
        name: Arc::from(func.name()),
        alias: alias_arc,
        columns: Arc::from(result.columns),
        rows: Arc::from(result.rows),
    }))
}
