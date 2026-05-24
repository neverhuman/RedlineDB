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

#[path = "cte_registry.rs"]
pub(crate) mod registry;

#[path = "cte_recursive.rs"]
mod recursive;

use std::collections::HashMap;
use std::sync::Arc;

use sqlparser::ast::With;

use crate::connection::Connection;
use crate::error::Result;
use crate::statement::{
    BoundTable, ParamLayout, PreparedKind, PreparedTemplate, SelectPlan, SelectSource,
};
use crate::value::SqlValue;
use redlinedb_kernel::catalog::{
    Affinity, ColumnDef, ColumnId, SchemaEpoch, SchemaId, SchemaSnapshot, TableDef, TableId,
};
use redlinedb_kernel::format::RelId;

use registry::register_cte_rows;
pub(crate) use registry::{register_external_rows, rows_for_relation};
use sqlparser::ast::Query;

/// Sentinel relation id used by synthetic CTE table defs. Real relations
/// allocate from a monotonic counter starting at 1, so picking the top
/// bit of u64 keeps us comfortably out of any plausible real-id range.
const CTE_RELATION_TAG: u64 = 0xC7E0_0000_0000_0000;

/// Returns true if this `TableDef` was synthesized for a CTE, a view,
/// or a cross-database alias (all three share the same row-storage
/// backing via `register_external_rows` and `rows_for_relation`). The
/// view, CTE, and cross-DB tags share the same row-registry namespace.
pub(crate) fn is_cte_table_def(def: &TableDef) -> bool {
    let tag = def.relation_id.0 & 0xFFFF_0000_0000_0000;
    tag == CTE_RELATION_TAG
        || tag == super::view::VIEW_RELATION_TAG
        || tag == super::cross_db::CROSS_DB_RELATION_TAG
}

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
    /// statement's thread. Reset before every top-level `bind_with_query`
    /// call so ids are stable per query plan.
    static CTE_REL_COUNTER: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    /// Per-thread name → CteDef registry that survives scope teardown.
    /// `bind_with_query` populates this when it materializes CTEs so
    /// subqueries that bind at exec time (after the scope stack has
    /// been popped) can still resolve CTE references by name. Cleared
    /// at the start of every new top-level `bind_with_query` call.
    static CTE_PERMANENT: std::cell::RefCell<HashMap<String, CteDef>> =
        std::cell::RefCell::new(HashMap::new());
}

fn register_permanent_cte(name: String, def: CteDef) {
    CTE_PERMANENT.with(|cell| {
        cell.borrow_mut().insert(name.to_ascii_lowercase(), def);
    });
}

fn clear_permanent_ctes() {
    CTE_PERMANENT.with(|cell| cell.borrow_mut().clear());
}

fn lookup_permanent_cte(name: &str) -> Option<CteDef> {
    CTE_PERMANENT.with(|cell| {
        let map = cell.borrow();
        map.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    })
}

fn next_cte_rel_id() -> RelId {
    CTE_REL_COUNTER.with(|cell| {
        let n = cell.get() + 1;
        cell.set(n);
        RelId(CTE_RELATION_TAG | n)
    })
}

/// Build a synthetic `TableDef` for a CTE so the join executor can treat
/// the CTE name as a real table. Column types are inferred from the
/// first non-NULL value in each column; declared NOT NULL is left off so
/// any value flows.
pub(crate) fn synth_table_def(
    name: &str,
    columns: &[String],
    rows: &[Vec<SqlValue>],
) -> Arc<TableDef> {
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
            generated: None,
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
        foreign_keys: Vec::new(),
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

/// Build a [`CteDef`] from a pre-materialized row set, registering the
/// row payload in the thread-local relation store so the join executor
/// can resolve the synthetic relation_id at execution time.
pub(crate) fn build_cte_def_from_rows(
    name: &str,
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
) -> CteDef {
    let table_def = synth_table_def(name, &columns, &rows);
    let rows_arc: Arc<Vec<Vec<SqlValue>>> = Arc::new(rows);
    register_cte_rows(table_def.relation_id, Arc::clone(&rows_arc));
    let row_slice: Arc<[Vec<SqlValue>]> = Arc::from(rows_arc.as_slice().to_vec());
    CteDef {
        name: Arc::from(name),
        columns: Arc::from(columns),
        rows: row_slice,
        table_def: Some(table_def),
    }
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
/// trailing query. The scope is popped before returning; we *also*
/// publish each CTE's name into `CTE_PERMANENT_NAMES` so subqueries
/// that bind at exec time (after the scope stack has been torn down)
/// can still resolve the name to its pre-materialized rows.
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

    CTE_REL_COUNTER.with(|cell| cell.set(0));
    // Clear stale permanent entries from any previous top-level
    // statement before binding the new one.
    clear_permanent_ctes();
    let mut pushed_scopes = 0usize;
    for cte in cte_tables {
        let def = recursive::materialize_cte(
            conn,
            Arc::clone(&schema),
            schema_epoch,
            sql,
            &cte,
            recursive,
        )?;
        register_permanent_cte(def.name.to_string(), def.clone());
        let mut single = HashMap::new();
        single.insert(def.name.to_string(), def);
        push_scope(single);
        pushed_scopes += 1;
    }
    let bound = super::super::parser::bind_query(conn, schema, schema_epoch, sql, body_query);
    for _ in 0..pushed_scopes {
        pop_scope();
    }
    bound
}

/// Run a Query and materialize all output rows. Returns rows plus column names.
pub(crate) fn run_query_to_rows(
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

/// Try to interpret a FROM table reference as a CTE. Returns a
/// `SelectSource::Cte` if the name matches a CTE in the active scope
/// or, failing that, in the per-thread permanent CTE registry (for
/// subqueries that bind at exec time after the scope stack has been
/// torn down).
pub(crate) fn try_resolve_cte_source(
    name: &sqlparser::ast::ObjectName,
    alias: Option<&Arc<str>>,
    _params: &mut ParamLayout,
) -> Option<SelectSource> {
    let last = name.0.last()?;
    let ident_name = match last {
        sqlparser::ast::ObjectNamePart::Identifier(ident) => &ident.value,
        _ => return None,
    };
    let def = resolve_cte_def(ident_name)?;
    Some(SelectSource::Cte {
        name: def.name,
        alias: alias.cloned(),
        columns: def.columns,
        rows: def.rows,
    })
}

/// Try to interpret a FROM/JOIN table reference as a CTE table. Returns
/// a synthetic `BoundTable` whose `TableDef.relation_id` is registered
/// in the global CTE row map.
pub(crate) fn try_resolve_cte_bound_table(
    name: &sqlparser::ast::ObjectName,
    alias: Option<&Arc<str>>,
) -> Option<BoundTable> {
    let last = name.0.last()?;
    let ident_name = match last {
        sqlparser::ast::ObjectNamePart::Identifier(ident) => &ident.value,
        _ => return None,
    };
    let def = resolve_cte_def(ident_name)?;
    let table = def.table_def?;
    Some(BoundTable {
        table,
        alias: alias.cloned(),
    })
}

/// Single resolution point that consults both scope tiers in a
/// deterministic order: active scope (the local `WITH` we are
/// currently binding) wins over the permanent registry (an enclosing
/// `WITH` whose scope has already been popped — used by subqueries
/// that bind at exec time).
fn resolve_cte_def(ident_name: &str) -> Option<CteDef> {
    if scope_active() {
        if let Some(def) = lookup(ident_name) {
            return Some(def);
        }
    }
    lookup_permanent_cte(ident_name)
}

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
        distinct_on: Vec::new(),
        projection: Vec::new(),
        selection: None,
        group_by: Vec::new(),
        having: None,
        order_by: Vec::new(),
        limit: None,
        offset: None,
    }
}

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
            distinct_on: Vec::new(),
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
use sqlparser::ast::Distinct;
