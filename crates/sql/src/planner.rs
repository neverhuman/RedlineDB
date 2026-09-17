#![allow(dead_code)]

use std::cmp::Ordering;
use std::fmt::Write as _;
use std::sync::Arc;

use redlinedb_kernel::catalog::{IndexDef, TableDef, TableStats};
use redlinedb_kernel::format::RowId;
use sqlparser::ast::{
    BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, OrderByExpr, SelectItem,
};

use crate::connection::{Connection, OptimizerConfig};
use crate::error::{Error, Result};
use crate::statement::{BoundTable, ExplainFormat, PreparedKind, SelectPlan, SelectSource};
use crate::value::SqlValue;

pub(crate) mod helpers;
use helpers::*;

mod access;
pub mod access_path;
mod build;
mod optimize;
mod policy;
mod trace;

use access::*;
#[allow(unused_imports)]
pub(crate) use access_path::AccessPath as AccessPathIr;
#[allow(unused_imports)]
pub(crate) use access_path::{
    OrderSatisfies as AccessPathOrderSatisfies, choose_access_path as choose_access_path_ir,
    lower_to_legacy as lower_access_path_to_legacy, planner_use_access_path,
    set_planner_use_access_path,
};
use build::*;
use optimize::*;
use policy::*;

const SEQ_PAGE_COST: f64 = 1.0;
const RANDOM_PAGE_COST: f64 = 4.0;
const CPU_TUPLE_COST: f64 = 0.01;
const CPU_OPERATOR_COST: f64 = 0.0025;
const INDEX_PROBE_STARTUP: f64 = 2.0;
const UNKNOWN_EQ_SELECTIVITY: f64 = 0.10;
const UNKNOWN_RANGE_SELECTIVITY: f64 = 0.33;
const UNKNOWN_PREDICATE_SELECTIVITY: f64 = 0.333;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    pub startup: f64,
    pub total: f64,
    pub rows: f64,
    pub width: f64,
    pub memory_bytes: usize,
    pub spill_bytes: usize,
}

impl Cost {
    pub fn zero() -> Self {
        Self {
            startup: 0.0,
            total: 0.0,
            rows: 0.0,
            width: 0.0,
            memory_bytes: 0,
            spill_bytes: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LogicalPlan {
    Scan(LogicalScan),
    Filter {
        input: Box<LogicalPlan>,
        predicate: String,
    },
    Project {
        input: Box<LogicalPlan>,
        exprs: Vec<String>,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        kind: JoinKind,
        on: Vec<String>,
    },
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<String>,
        aggs: Vec<String>,
    },
    Sort {
        input: Box<LogicalPlan>,
        keys: Vec<String>,
    },
    Limit {
        input: Box<LogicalPlan>,
        limit: Option<String>,
        offset: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct LogicalScan {
    pub source: String,
    pub access: AccessPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    NestedLoop,
    IndexNestedLoop,
    Hash,
    Cross,
}

#[derive(Debug, Clone)]
pub enum AccessPath {
    TableScan,
    RowIdGet {
        rowid: RowId,
    },
    IndexPointLookup {
        index: Arc<IndexDef>,
        predicates: Vec<String>,
    },
    IndexRangeScan {
        index: Arc<IndexDef>,
        predicates: Vec<String>,
    },
    CoveringIndexScan {
        index: Arc<IndexDef>,
        predicates: Vec<String>,
        projected_columns: Vec<String>,
    },
    MultiIndexOr {
        inputs: Vec<AccessPath>,
    },
    MultiIndexAnd {
        inputs: Vec<AccessPath>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalKind {
    TableScan,
    RowIdGet,
    IndexScan,
    MultiIndexScan,
    Filter,
    Project,
    NestedLoopJoin,
    IndexNestedLoopJoin,
    HashJoin,
    StreamingAggregate,
    HashAggregate,
    Sort,
    TopN,
    Limit,
    Explain,
    Constant,
}

#[derive(Debug, Clone)]
pub struct PhysicalPlan {
    pub kind: PhysicalKind,
    pub relation: Option<String>,
    pub index: Option<String>,
    /// The B-tree probe kind for `PhysicalKind::IndexScan` nodes, used
    /// only for EXPLAIN rendering. `None` for non-index nodes and for
    /// `CoveringIndexScan` (which renders distinctly via the
    /// `projected_columns` marker today).
    pub index_probe_kind: Option<&'static str>,
    /// Phase 11 W1-D: when the index leading column matches the
    /// `ORDER BY` column AND a `LIMIT n` is in effect, the executor
    /// truncates the cursor walk after `n` visible rows. The planner
    /// records the limit here so EXPLAIN can render
    /// `IndexScan ... LIMIT n` and downstream consumers can spot
    /// the early-stop annotation. `None` means "drain the full
    /// range".
    pub ordered_index_scan_limit: Option<usize>,
    pub estimated_rows: f64,
    pub cost: Cost,
    pub access_predicates: Vec<String>,
    pub residual_predicates: Vec<String>,
    pub output_order: Vec<String>,
    pub projected_columns: Vec<String>,
    pub memory_budget: usize,
    pub actual_rows: Option<usize>,
    pub loops: Option<usize>,
    pub elapsed_ms: Option<f64>,
    pub peak_memory_bytes: Option<usize>,
    pub spill_bytes: Option<usize>,
    pub children: Vec<PhysicalPlan>,
}

impl PhysicalPlan {
    fn new(kind: PhysicalKind) -> Self {
        Self {
            kind,
            relation: None,
            index: None,
            index_probe_kind: None,
            ordered_index_scan_limit: None,
            estimated_rows: 0.0,
            cost: Cost::zero(),
            access_predicates: Vec::new(),
            residual_predicates: Vec::new(),
            output_order: Vec::new(),
            projected_columns: Vec::new(),
            memory_budget: 0,
            actual_rows: None,
            loops: None,
            elapsed_ms: None,
            peak_memory_bytes: None,
            spill_bytes: None,
            children: Vec::new(),
        }
    }

    fn leaf(kind: PhysicalKind, relation: Option<String>) -> Self {
        let mut plan = Self::new(kind);
        plan.relation = relation;
        plan
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExplainMetrics {
    pub actual_rows: Option<usize>,
    pub loops: Option<usize>,
    pub elapsed_ms: Option<f64>,
    pub peak_memory_bytes: Option<usize>,
    pub spill_bytes: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlattenedNode {
    pub(crate) id: usize,
    pub(crate) parent: Option<usize>,
    pub(crate) detail: String,
}

pub(crate) fn explain_rows(
    conn: &Connection,
    kind: &PreparedKind,
    bindings: &[Option<SqlValue>],
    metrics: Option<ExplainMetrics>,
    format: ExplainFormat,
) -> Vec<Vec<SqlValue>> {
    let plan = build_plan(conn, kind, bindings, metrics);
    trace::maybe_emit_planner_trace(&plan);
    match format {
        ExplainFormat::QueryPlan => flatten_query_plan(&plan)
            .into_iter()
            .map(|node| {
                vec![
                    SqlValue::Integer(node.id as i64),
                    SqlValue::Integer(node.parent.map(|id| id as i64).unwrap_or(0)),
                    SqlValue::Integer(0),
                    SqlValue::Text(Arc::from(node.detail)),
                ]
            })
            .collect(),
        ExplainFormat::Text => vec![vec![SqlValue::Text(Arc::from(render_text(&plan)))]],
        ExplainFormat::Json => vec![vec![SqlValue::Text(Arc::from(render_json(&plan)))]],
    }
}

pub(crate) fn build_plan(
    conn: &Connection,
    kind: &PreparedKind,
    bindings: &[Option<SqlValue>],
    metrics: Option<ExplainMetrics>,
) -> PhysicalPlan {
    let mut plan = match kind {
        PreparedKind::Select(select) => build_select_plan(conn, select, bindings),
        PreparedKind::Insert(plan) => simple_node(
            PhysicalKind::Constant,
            format!("INSERT INTO {}", plan.table.name),
        ),
        PreparedKind::InsertView(plan) => simple_node(
            PhysicalKind::Constant,
            format!("INSERT INTO {}", plan.view_name),
        ),
        PreparedKind::Update(plan) => simple_node(
            PhysicalKind::Constant,
            format!("UPDATE {}", plan.table.name),
        ),
        PreparedKind::Delete(plan) => simple_node(
            PhysicalKind::Constant,
            format!("DELETE FROM {}", plan.table.name),
        ),
        PreparedKind::Analyze(plan) => match &plan.table {
            Some(table) => simple_node(PhysicalKind::Constant, format!("ANALYZE {}", table.name)),
            None => simple_node(PhysicalKind::Constant, "ANALYZE".to_owned()),
        },
        PreparedKind::Explain(plan) => {
            build_plan(conn, &plan.inner.kind, bindings, metrics.clone())
        }
        PreparedKind::Begin(_) => simple_node(PhysicalKind::Constant, "BEGIN".to_owned()),
        PreparedKind::Commit => simple_node(PhysicalKind::Constant, "COMMIT".to_owned()),
        PreparedKind::Rollback => simple_node(PhysicalKind::Constant, "ROLLBACK".to_owned()),
        PreparedKind::CreateTable(_)
        | PreparedKind::CreateTempTable(_)
        | PreparedKind::CreateTableAsSelect(_)
        | PreparedKind::CreateVirtualTable(_) => {
            simple_node(PhysicalKind::Constant, "CREATE TABLE".to_owned())
        }
        PreparedKind::CreateIndex(_) => {
            simple_node(PhysicalKind::Constant, "CREATE INDEX".to_owned())
        }
        PreparedKind::DropTable(_) => simple_node(PhysicalKind::Constant, "DROP TABLE".to_owned()),
        PreparedKind::DropIndex(_) => simple_node(PhysicalKind::Constant, "DROP INDEX".to_owned()),
        PreparedKind::AlterTable(_) => {
            simple_node(PhysicalKind::Constant, "ALTER TABLE".to_owned())
        }
        PreparedKind::Pragma(_) => simple_node(PhysicalKind::Constant, "PRAGMA".to_owned()),
        PreparedKind::Attach(_)
        | PreparedKind::CrossDbSql(_)
        | PreparedKind::CrossDbInsertSelect(_) => {
            simple_node(PhysicalKind::Constant, "ATTACH/DETACH".to_owned())
        }
        PreparedKind::Reindex => simple_node(PhysicalKind::Constant, "REINDEX".to_owned()),
        PreparedKind::Vacuum => simple_node(PhysicalKind::Constant, "VACUUM".to_owned()),
        PreparedKind::VacuumInto { .. } => {
            simple_node(PhysicalKind::Constant, "VACUUM INTO".to_owned())
        }
        PreparedKind::CreateView(_) => {
            simple_node(PhysicalKind::Constant, "CREATE VIEW".to_owned())
        }
        PreparedKind::DropView(_) => simple_node(PhysicalKind::Constant, "DROP VIEW".to_owned()),
        PreparedKind::CreateTrigger(_) => {
            simple_node(PhysicalKind::Constant, "CREATE TRIGGER".to_owned())
        }
        PreparedKind::DropTrigger(_) => {
            simple_node(PhysicalKind::Constant, "DROP TRIGGER".to_owned())
        }
        PreparedKind::CreateSchema { .. } => {
            simple_node(PhysicalKind::Constant, "CREATE SCHEMA".to_owned())
        }
        PreparedKind::DropSchema { .. } => {
            simple_node(PhysicalKind::Constant, "DROP SCHEMA".to_owned())
        }
        PreparedKind::CreateSequence { .. } => {
            simple_node(PhysicalKind::Constant, "CREATE SEQUENCE".to_owned())
        }
        PreparedKind::DropSequence { .. } => {
            simple_node(PhysicalKind::Constant, "DROP SEQUENCE".to_owned())
        }
        PreparedKind::SetTransactionIsolation { .. } => {
            simple_node(PhysicalKind::Constant, "SET TRANSACTION".to_owned())
        }
        PreparedKind::ShowVariable { .. } => simple_node(PhysicalKind::Constant, "SHOW".to_owned()),
        PreparedKind::AlterIndex { .. } => {
            simple_node(PhysicalKind::Constant, "ALTER INDEX".to_owned())
        }
        PreparedKind::Merge(plan) => simple_node(
            PhysicalKind::Constant,
            format!("MERGE INTO {}", plan.target.name),
        ),
    };

    if let Some(metrics) = metrics {
        plan.actual_rows = metrics.actual_rows;
        plan.loops = metrics.loops;
        plan.elapsed_ms = metrics.elapsed_ms;
        plan.peak_memory_bytes = metrics.peak_memory_bytes;
        plan.spill_bytes = metrics.spill_bytes;
    }
    plan
}

fn simple_node(kind: PhysicalKind, detail: String) -> PhysicalPlan {
    let mut node = PhysicalPlan::new(kind);
    node.relation = Some(detail);
    node
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod planner_conservatism_tests {
    use super::*;

    /// The executor must be able to consume every variant the planner
    /// could emit today. Extend the match arms in
    /// `access_path_is_consumable_by_executor` when adding a new
    /// variant — and add a paired executor arm before flipping the
    /// answer to `true`.
    #[test]
    fn planner_only_emits_executor_consumable_variants() {
        // TableScan is consumable.
        assert!(access_path_is_consumable_by_executor(
            &AccessPath::TableScan
        ));
        // RowIdGet is consumable.
        assert!(access_path_is_consumable_by_executor(
            &AccessPath::RowIdGet {
                rowid: redlinedb_kernel::format::RowId::new(1),
            }
        ));
    }
}
