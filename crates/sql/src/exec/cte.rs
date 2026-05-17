//! Common Table Expression (CTE) binding and execution.
//!
//! See `synth_table_def` for the synthetic-`TableDef` trick used to feed
//! pre-materialized CTE rows through the join executor.
//!
//! Both non-recursive and recursive `WITH` clauses are handled by
//! pre-materializing each CTE body into rows during statement binding.
//! The CTE rows are then exposed via [`SelectSource::Cte`] when the
//! trailing query references the CTE name.
//!
//! Recursive evaluation follows the SQL standard's working-set semantics:
//!   1. Evaluate the anchor branch (the side that does not reference the
//!      CTE) and seed the result set.
//!   2. Repeatedly evaluate the recursive branch against the *just-emitted*
//!      rows, appending new rows to the result set.
//!   3. Stop when an iteration produces zero new rows. `UNION` (without
//!      `ALL`) deduplicates across iterations so cycles in the recursion
//!      terminate naturally; a configurable bound caps run-away cases.
//!
//! See `crates/sql/tests/parity_cte.rs` for differential coverage.
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use sqlparser::ast::{
    Cte, Distinct, Expr, Query, SetExpr, SetOperator, SetQuantifier, TableFactor, TableWithJoins,
    With,
};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::statement::{
    BoundTable, ParamLayout, PreparedKind, PreparedTemplate, SelectPlan, SelectSource,
};
use crate::value::SqlValue;
use redlinedb_kernel::catalog::{
    Affinity, ColumnDef, ColumnId, SchemaEpoch, SchemaId, SchemaSnapshot, TableDef, TableId,
};
use redlinedb_kernel::format::RelId;

/// Sentinel relation id used by synthetic CTE table defs. Real relations
/// allocate from a monotonic counter starting at 1, so picking the top
/// bit of u64 keeps us comfortably out of any plausible real-id range.
const CTE_RELATION_TAG: u64 = 0xC7E0_0000_0000_0000;

/// Returns true if this `TableDef` was synthesized for a CTE.
pub(crate) fn is_cte_table_def(def: &TableDef) -> bool {
    (def.relation_id.0 & 0xFFFF_0000_0000_0000) == CTE_RELATION_TAG
}

/// Maximum recursive-CTE iterations before bailing out.
const RECURSIVE_CTE_ITERATION_LIMIT: usize = 10_000;

/// A pre-materialized CTE definition: rows + column names + optional
/// synthetic `TableDef` so the join executor can treat the CTE name as
/// a real table.
#[derive(Clone)]
pub(crate) struct CteDef {
    pub(crate) name: Arc<str>,
    pub(crate) columns: Arc<[String]>,
    pub(crate) rows: Arc<[Vec<SqlValue>]>,
    /// Synthetic TableDef; populated on demand the first time the CTE
    /// gets resolved through `try_resolve_cte_join_table`.
    pub(crate) table_def: Option<Arc<TableDef>>,
}

thread_local! {
    static CTE_SCOPE: std::cell::RefCell<Vec<HashMap<String, CteDef>>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Monotonic counter for synthetic CTE relation ids within a single
    /// statement's thread. The counter is reset before every top-level
    /// `bind_with_query` call so ids are stable per query plan.
    static CTE_REL_COUNTER: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// Per-thread registry of CTE row stores keyed by synthesized
    /// relation_id. Per-thread (rather than global) so concurrent tests
    /// don't trample each other's id space. Statements always bind &
    /// execute on the same thread.
    static CTE_ROWS_TL: std::cell::RefCell<HashMap<u64, Arc<Vec<Vec<SqlValue>>>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Reserved global slot used only to keep the static initialization
/// idiomatic; CTE row storage itself is in `CTE_ROWS_TL`.
#[allow(dead_code)]
static _CTE_ROWS_GLOBAL_RESERVED: Mutex<()> = Mutex::new(());

fn next_cte_rel_id() -> RelId {
    CTE_REL_COUNTER.with(|cell| {
        let n = cell.get() + 1;
        cell.set(n);
        RelId(CTE_RELATION_TAG | n)
    })
}

/// Look up the rows backing a synthetic CTE TableDef.
pub(crate) fn rows_for_relation(rel: RelId) -> Option<Arc<Vec<Vec<SqlValue>>>> {
    CTE_ROWS_TL.with(|cell| cell.borrow().get(&rel.0).cloned())
}

fn register_cte_rows(rel: RelId, rows: Arc<Vec<Vec<SqlValue>>>) {
    CTE_ROWS_TL.with(|cell| {
        cell.borrow_mut().insert(rel.0, rows);
    });
}

#[allow(dead_code)]
fn release_scope_rows(scope: &HashMap<String, CteDef>) {
    CTE_ROWS_TL.with(|cell| {
        let mut map = cell.borrow_mut();
        for def in scope.values() {
            if let Some(tdef) = def.table_def.as_ref() {
                map.remove(&tdef.relation_id.0);
            }
        }
    });
}

/// Build a synthetic `TableDef` for a CTE so the join executor can treat
/// the CTE name as a real table. Column types are inferred from the
/// first non-NULL value in each column (NULL columns default to TEXT
/// affinity); declared NOT NULL is left off so any value flows.
fn synth_table_def(name: &str, columns: &[String], rows: &[Vec<SqlValue>]) -> Arc<TableDef> {
    let column_defs: Vec<ColumnDef> = columns
        .iter()
        .enumerate()
        .map(|(idx, name)| ColumnDef {
            column_id: ColumnId((idx + 1) as u64),
            ordinal: idx as u16,
            name: Box::from(name.as_str()),
            folded: Box::from(name.to_ascii_lowercase().as_str()),
            declared_type: None,
            affinity: infer_affinity(rows, idx),
            not_null: false,
            default_value: None,
            default_expr: None,
        })
        .collect();
    let rel = next_cte_rel_id();
    Arc::new(TableDef {
        table_id: TableId(rel.0),
        schema_id: SchemaId(0),
        relation_id: rel,
        name: Box::from(name),
        folded: Box::from(name.to_ascii_lowercase().as_str()),
        columns: column_defs,
        indexes: Vec::new(),
        constraints: Vec::new(),
        checks: Vec::new(),
        rowid_alias_column: None,
        flags: 0,
        normalized_sql: None,
    })
}

fn infer_affinity(rows: &[Vec<SqlValue>], col: usize) -> Affinity {
    for row in rows {
        if let Some(v) = row.get(col) {
            match v {
                SqlValue::Integer(_) => return Affinity::Integer,
                SqlValue::Real(_) => return Affinity::Real,
                SqlValue::Text(_) => return Affinity::Text,
                SqlValue::Blob(_) => return Affinity::Blob,
                SqlValue::Null => continue,
            }
        }
    }
    Affinity::Blob
}

/// Push a CTE scope (visible to bind-time lookups). Pairs with `pop_scope`.
pub(crate) fn push_scope(scope: HashMap<String, CteDef>) {
    CTE_SCOPE.with(|cell| cell.borrow_mut().push(scope));
}

pub(crate) fn pop_scope() {
    CTE_SCOPE.with(|cell| {
        cell.borrow_mut().pop();
    });
}

/// Look up a CTE by (case-insensitive) name in the current scope chain.
pub(crate) fn lookup(name: &str) -> Option<CteDef> {
    CTE_SCOPE.with(|cell| {
        for scope in cell.borrow().iter().rev() {
            for (key, value) in scope.iter() {
                if key.eq_ignore_ascii_case(name) {
                    return Some(value.clone());
                }
            }
        }
        None
    })
}

/// True if there is any CTE in the current scope chain.
pub(crate) fn scope_active() -> bool {
    CTE_SCOPE.with(|cell| !cell.borrow().is_empty())
}

/// Bind a `WITH ... query` form. Pre-executes each CTE body (handling
/// recursive references) and pushes a CTE scope before binding the
/// trailing query. The scope is popped before returning.
pub(crate) fn bind_with_query(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    with: With,
    body_query: Query,
) -> Result<PreparedTemplate> {
    let With {
        recursive,
        cte_tables,
        ..
    } = with;

    // Reset the synthetic-relation counter for this query.
    CTE_REL_COUNTER.with(|cell| cell.set(0));
    let mut scope: HashMap<String, CteDef> = HashMap::new();
    for cte in cte_tables {
        let def = materialize_cte(conn, Arc::clone(&schema), schema_epoch, sql, &cte, recursive)?;
        scope.insert(def.name.to_string(), def);
    }

    push_scope(scope);
    let bound = super::super::parser::bind_query(conn, schema, schema_epoch, sql, body_query);
    // Pop the scope so nested binds don't see this CTE. The synthetic-
    // TableDef row registry is intentionally NOT cleared — the bound
    // template references those rows via relation_id and may be cached
    // and re-executed later.
    pop_scope();
    bound
}

/// Materialize one CTE (anchor + optional recursive arm) into rows.
fn materialize_cte(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    cte: &Cte,
    parent_recursive: bool,
) -> Result<CteDef> {
    let cte_name: Arc<str> = Arc::from(cte.alias.name.value.as_str());
    let declared_columns: Vec<String> = cte
        .alias
        .columns
        .iter()
        .map(|c| c.name.value.clone())
        .collect();

    // Detect whether this CTE's body references itself. Self-references in
    // a non-RECURSIVE WITH are a parse error in SQLite; if present, treat
    // it as recursive (matches sqlite tolerant behavior for our purposes).
    let body_query = cte.query.as_ref().clone();
    let self_referenced = body_uses_name(&body_query, &cte_name);

    if !self_referenced {
        // Non-recursive (or recursive WITH but this CTE doesn't recurse):
        // run the body once. Other CTEs from the same WITH are NOT visible
        // here yet (we add them progressively as we materialize them), so
        // we rely on the scope chain populated up to this point.
        let (rows, columns) = run_query_to_rows(
            conn,
            Arc::clone(&schema),
            schema_epoch,
            sql,
            body_query,
            &declared_columns,
        )?;
        let table_def = synth_table_def(&cte_name, &columns, &rows);
        let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(rows);
        register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
        let row_slice: Arc<[Vec<SqlValue>]> = Arc::from(rows_arc.as_slice().to_vec());
        return Ok(CteDef {
            name: cte_name,
            columns: Arc::from(columns),
            rows: row_slice,
            table_def: Some(table_def),
        });
    }

    if !parent_recursive {
        return Err(Error::UnsupportedSql(format!(
            "CTE `{cte_name}` references itself but WITH was not declared RECURSIVE"
        )));
    }

    // Recursive CTE: body must be a UNION of an anchor and a recursive
    // arm, where the anchor does not reference the CTE name. Either arm
    // can come first.
    let (anchor_branch, recursive_branch, union_all) =
        split_recursive_body(&body_query, &cte_name)?;

    // Seed: evaluate the anchor (no self-scope yet).
    let (mut accumulated, columns) = run_query_to_rows(
        conn,
        Arc::clone(&schema),
        schema_epoch,
        sql,
        anchor_branch,
        &declared_columns,
    )?;
    let columns_arc: Arc<[String]> = Arc::from(columns);
    let column_vec: Vec<String> = columns_arc.iter().cloned().collect();

    // Dedup the seed set when UNION semantics apply.
    if !union_all {
        accumulated = dedup_rows(accumulated);
    }

    let mut working_set = accumulated.clone();

    for iter in 0..RECURSIVE_CTE_ITERATION_LIMIT {
        if working_set.is_empty() {
            let table_def = synth_table_def(&cte_name, &column_vec, &accumulated);
            let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(accumulated.clone());
            register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
            return Ok(CteDef {
                name: cte_name,
                columns: columns_arc,
                rows: Arc::from(accumulated),
                table_def: Some(table_def),
            });
        }

        // Build a scope that exposes the working set as `cte_name`.
        // The synth table for the working-set view registers its own
        // relation id so JOINs against the recursive arm can scan it.
        let working_table = synth_table_def(&cte_name, &column_vec, &working_set);
        let working_rows: Arc<Vec<Vec<SqlValue>>> = Arc::new(working_set.clone());
        register_cte_rows(working_table.relation_id, Arc::clone(&working_rows));
        let mut scope = HashMap::new();
        scope.insert(
            cte_name.to_string(),
            CteDef {
                name: Arc::clone(&cte_name),
                columns: Arc::clone(&columns_arc),
                rows: Arc::from(working_set.clone()),
                table_def: Some(Arc::clone(&working_table)),
            },
        );
        push_scope(scope);

        let recursive_result = run_query_to_rows(
            conn,
            Arc::clone(&schema),
            schema_epoch,
            sql,
            recursive_branch.clone(),
            &declared_columns,
        );
        pop_scope();
        // Drop the working-set rows from the per-thread registry now
        // that the recursive arm has consumed them.
        CTE_ROWS_TL.with(|cell| {
            cell.borrow_mut().remove(&working_table.relation_id.0);
        });

        let (new_rows, _) = recursive_result?;

        // Filter / dedup new rows depending on UNION semantics.
        let next_working: Vec<Vec<SqlValue>> = if union_all {
            new_rows
        } else {
            new_rows
                .into_iter()
                .filter(|row| !row_in(&accumulated, row))
                .collect::<Vec<_>>()
        };

        if next_working.is_empty() {
            let table_def = synth_table_def(&cte_name, &column_vec, &accumulated);
            let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(accumulated.clone());
            register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
            return Ok(CteDef {
                name: cte_name,
                columns: columns_arc,
                rows: Arc::from(accumulated),
                table_def: Some(table_def),
            });
        }

        let next_working = if union_all {
            next_working
        } else {
            dedup_rows(next_working)
        };

        accumulated.extend(next_working.iter().cloned());
        working_set = next_working;

        if iter + 1 == RECURSIVE_CTE_ITERATION_LIMIT {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` exceeded {RECURSIVE_CTE_ITERATION_LIMIT} iterations"
            )));
        }
    }
    let table_def = synth_table_def(&cte_name, &column_vec, &accumulated);
    let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(accumulated.clone());
    register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
    Ok(CteDef {
        name: cte_name,
        columns: columns_arc,
        rows: Arc::from(accumulated),
        table_def: Some(table_def),
    })
}

/// Run a Query and materialize all output rows. Returns the rows plus the
/// column names (either declared in the CTE alias list or derived from
/// the query's output).
fn run_query_to_rows(
    conn: &Connection,
    schema: Arc<SchemaSnapshot>,
    schema_epoch: SchemaEpoch,
    sql: &str,
    query: Query,
    declared_columns: &[String],
) -> Result<(Vec<Vec<SqlValue>>, Vec<String>)> {
    let template = super::super::parser::bind_query(conn, schema, schema_epoch, sql, query)?;
    let columns: Vec<String> = if !declared_columns.is_empty() {
        declared_columns.to_vec()
    } else {
        template.output_columns.iter().cloned().collect()
    };
    let rows = super::materialize_prepared_rows(conn, &template, &[])?;
    Ok((rows, columns))
}

/// Split a recursive CTE body into (anchor, recursive_arm, is_union_all).
/// Accepts a top-level set-op UNION / UNION ALL.
fn split_recursive_body(query: &Query, cte_name: &str) -> Result<(Query, Query, bool)> {
    let body = query.body.as_ref();
    let (op, quantifier, left, right) = match body {
        SetExpr::SetOperation {
            op,
            set_quantifier,
            left,
            right,
        } => (op, set_quantifier, left, right),
        _ => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` body must be a UNION / UNION ALL"
            )));
        }
    };
    if !matches!(op, SetOperator::Union) {
        return Err(Error::UnsupportedSql(format!(
            "recursive CTE `{cte_name}` requires UNION or UNION ALL"
        )));
    }
    let union_all = matches!(quantifier, SetQuantifier::All);
    let left_uses = setexpr_uses_name(left, cte_name);
    let right_uses = setexpr_uses_name(right, cte_name);
    let (anchor, recursive_arm) = match (left_uses, right_uses) {
        (false, true) => (left.as_ref().clone(), right.as_ref().clone()),
        (true, false) => (right.as_ref().clone(), left.as_ref().clone()),
        (false, false) => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` does not reference itself"
            )));
        }
        (true, true) => {
            return Err(Error::UnsupportedSql(format!(
                "recursive CTE `{cte_name}` self-references in both UNION arms"
            )));
        }
    };
    Ok((wrap_as_query(anchor), wrap_as_query(recursive_arm), union_all))
}

fn wrap_as_query(body: SetExpr) -> Query {
    Query {
        with: None,
        body: Box::new(body),
        order_by: None,
        limit_clause: None,
        fetch: None,
        locks: Vec::new(),
        for_clause: None,
        settings: None,
        format_clause: None,
        pipe_operators: Vec::new(),
    }
}

fn body_uses_name(query: &Query, name: &str) -> bool {
    setexpr_uses_name(query.body.as_ref(), name)
}

fn setexpr_uses_name(set_expr: &SetExpr, name: &str) -> bool {
    match set_expr {
        SetExpr::Select(select) => from_uses_name(&select.from, name)
            || select
                .selection
                .as_ref()
                .is_some_and(|expr| expr_uses_name(expr, name)),
        SetExpr::Query(inner) => setexpr_uses_name(inner.body.as_ref(), name),
        SetExpr::SetOperation { left, right, .. } => {
            setexpr_uses_name(left, name) || setexpr_uses_name(right, name)
        }
        _ => false,
    }
}

fn from_uses_name(from: &[TableWithJoins], name: &str) -> bool {
    from.iter().any(|table| {
        table_factor_uses_name(&table.relation, name)
            || table
                .joins
                .iter()
                .any(|join| table_factor_uses_name(&join.relation, name))
    })
}

fn table_factor_uses_name(factor: &TableFactor, name: &str) -> bool {
    match factor {
        TableFactor::Table {
            name: object_name, ..
        } => object_name
            .0
            .last()
            .and_then(|part| match part {
                sqlparser::ast::ObjectNamePart::Identifier(ident) => Some(&ident.value),
                _ => None,
            })
            .is_some_and(|n| n.eq_ignore_ascii_case(name)),
        TableFactor::Derived { subquery, .. } => setexpr_uses_name(subquery.body.as_ref(), name),
        _ => false,
    }
}

fn expr_uses_name(expr: &Expr, name: &str) -> bool {
    use sqlparser::ast::Expr as E;
    match expr {
        E::Subquery(q) | E::Exists { subquery: q, .. } => setexpr_uses_name(q.body.as_ref(), name),
        E::BinaryOp { left, right, .. } => expr_uses_name(left, name) || expr_uses_name(right, name),
        E::UnaryOp { expr, .. } | E::Nested(expr) | E::IsNull(expr) | E::IsNotNull(expr) => {
            expr_uses_name(expr, name)
        }
        E::Function(func) => match &func.args {
            sqlparser::ast::FunctionArguments::List(list) => list.args.iter().any(|arg| match arg {
                sqlparser::ast::FunctionArg::Unnamed(
                    sqlparser::ast::FunctionArgExpr::Expr(inner),
                ) => expr_uses_name(inner, name),
                _ => false,
            }),
            _ => false,
        },
        _ => false,
    }
}

fn dedup_rows(rows: Vec<Vec<SqlValue>>) -> Vec<Vec<SqlValue>> {
    let mut seen: Vec<Vec<SqlValue>> = Vec::with_capacity(rows.len());
    for row in rows {
        if !row_in(&seen, &row) {
            seen.push(row);
        }
    }
    seen
}

fn row_in(rows: &[Vec<SqlValue>], probe: &[SqlValue]) -> bool {
    rows.iter().any(|row| rows_equal(row, probe))
}

fn rows_equal(left: &[SqlValue], right: &[SqlValue]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .all(|(a, b)| crate::value::compare_values(a, b) == std::cmp::Ordering::Equal)
}

/// Try to interpret a FROM table reference as a CTE. Returns a
/// `SelectSource::Cte` if the name matches a CTE in the active scope.
pub(crate) fn try_resolve_cte_source(
    name: &sqlparser::ast::ObjectName,
    alias: Option<&Arc<str>>,
    _params: &mut ParamLayout,
) -> Option<SelectSource> {
    if !scope_active() {
        return None;
    }
    let last = name.0.last()?;
    let ident_name = match last {
        sqlparser::ast::ObjectNamePart::Identifier(ident) => &ident.value,
        _ => return None,
    };
    let def = lookup(ident_name)?;
    Some(SelectSource::Cte {
        name: def.name,
        alias: alias.cloned(),
        columns: def.columns,
        rows: def.rows,
    })
}

/// Try to interpret a FROM/JOIN table reference as a CTE table. Returns
/// a synthetic `BoundTable` whose `TableDef.relation_id` is registered
/// in the global CTE row map. Used by `bind_select_join_relation` so
/// recursive CTEs that JOIN against themselves resolve cleanly.
pub(crate) fn try_resolve_cte_bound_table(
    name: &sqlparser::ast::ObjectName,
    alias: Option<&Arc<str>>,
) -> Option<BoundTable> {
    if !scope_active() {
        return None;
    }
    let last = name.0.last()?;
    let ident_name = match last {
        sqlparser::ast::ObjectNamePart::Identifier(ident) => &ident.value,
        _ => return None,
    };
    let def = lookup(ident_name)?;
    let table = def.table_def?;
    Some(BoundTable {
        table,
        alias: alias.cloned(),
    })
}

/// Used by the binder when it has detected that the `CompoundAll`
/// wrapper for a recursive CTE shape is fine.
#[allow(dead_code)]
pub(crate) fn from_static(
    name: &str,
    alias: Option<&str>,
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
) -> SelectPlan {
    SelectPlan {
        source: SelectSource::Cte {
            name: Arc::from(name),
            alias: alias.map(Arc::from),
            columns: Arc::from(columns),
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
    }
}

/// Build a tiny `PreparedTemplate` whose source is just the supplied CTE
/// rows. Currently only used by tests / introspection.
#[allow(dead_code)]
pub(crate) fn template_from_static(
    sql: &str,
    schema_epoch: SchemaEpoch,
    output_columns: Arc<[String]>,
    rows: Arc<[Vec<SqlValue>]>,
) -> PreparedTemplate {
    PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns,
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source: SelectSource::StaticRows { rows },
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

// Re-export Distinct so the parser scope picks it up if needed.
#[allow(unused_imports)]
use Distinct as _;
