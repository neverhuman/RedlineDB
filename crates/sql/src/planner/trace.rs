use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

use serde_json::json;

use super::{PhysicalKind, PhysicalPlan, planner_use_access_path};

const TRACE_DIR_ENV: &str = "REDLINEDB_PLANNER_TRACE_DIR";
const TRACE_FILE: &str = "planner-trace.jsonl";

pub(crate) fn maybe_emit_planner_trace(plan: &PhysicalPlan) {
    let Some(dir) = std::env::var_os(TRACE_DIR_ENV) else {
        return;
    };
    let _ = emit_planner_trace(Path::new(&dir), plan);
}

fn emit_planner_trace(dir: &Path, plan: &PhysicalPlan) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(TRACE_FILE);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let record = trace_record(plan);
    let mut row = serde_json::to_vec(&record)?;
    row.push(b'\n');
    file.write_all(&row)
}

fn trace_record(plan: &PhysicalPlan) -> serde_json::Value {
    let access = first_access_node(plan);
    let hard_limit = first_ordered_index_limit(plan);
    json!({
        "trace_version": 1,
        "planner": {
            "access_path_gate": planner_use_access_path(),
        },
        "plan": {
            "root_kind": physical_kind_label(plan.kind),
            "sort_required": contains_kind(plan, PhysicalKind::Sort) || contains_kind(plan, PhysicalKind::TopN),
            "limit_pushdown": hard_limit,
        },
        "chosen_access": access.map(|node| json!({
            "kind": physical_kind_label(node.kind),
            "relation": node.relation,
            "index": node.index,
            "index_probe_kind": node.index_probe_kind,
            "access_predicates": node.access_predicates,
            "residual_predicates": node.residual_predicates,
            "output_order": node.output_order,
            "covering": false,
            "estimated_rows": node.estimated_rows,
            "total_cost": node.cost.total,
        })),
        "rejected_paths_complete": false,
    })
}

fn first_access_node(plan: &PhysicalPlan) -> Option<&PhysicalPlan> {
    if matches!(
        plan.kind,
        PhysicalKind::TableScan
            | PhysicalKind::RowIdGet
            | PhysicalKind::IndexScan
            | PhysicalKind::MultiIndexScan
    ) {
        return Some(plan);
    }
    plan.children.iter().find_map(first_access_node)
}

fn contains_kind(plan: &PhysicalPlan, kind: PhysicalKind) -> bool {
    plan.kind == kind || plan.children.iter().any(|child| contains_kind(child, kind))
}

fn first_ordered_index_limit(plan: &PhysicalPlan) -> Option<usize> {
    plan.ordered_index_scan_limit
        .or_else(|| plan.children.iter().find_map(first_ordered_index_limit))
}

fn physical_kind_label(kind: PhysicalKind) -> &'static str {
    match kind {
        PhysicalKind::TableScan => "TableScan",
        PhysicalKind::RowIdGet => "RowIdGet",
        PhysicalKind::IndexScan => "IndexScan",
        PhysicalKind::MultiIndexScan => "MultiIndexScan",
        PhysicalKind::Filter => "Filter",
        PhysicalKind::Project => "Project",
        PhysicalKind::NestedLoopJoin => "NestedLoopJoin",
        PhysicalKind::IndexNestedLoopJoin => "IndexNestedLoopJoin",
        PhysicalKind::HashJoin => "HashJoin",
        PhysicalKind::StreamingAggregate => "StreamingAggregate",
        PhysicalKind::HashAggregate => "HashAggregate",
        PhysicalKind::Sort => "Sort",
        PhysicalKind::TopN => "TopN",
        PhysicalKind::Limit => "Limit",
        PhysicalKind::Explain => "Explain",
        PhysicalKind::Constant => "Constant",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::Cost;

    fn index_scan_plan() -> PhysicalPlan {
        let mut index = PhysicalPlan::leaf(PhysicalKind::IndexScan, Some("kv".to_owned()));
        index.index = Some("kv_tk".to_owned());
        index.index_probe_kind = Some("RangeScan");
        index.access_predicates = vec!["USING INDEX kv_tk (RangeScan)".to_owned()];
        index.output_order = vec!["k".to_owned()];
        index.ordered_index_scan_limit = Some(3);
        index.estimated_rows = 8.0;
        index.cost = Cost {
            startup: 2.0,
            total: 3.5,
            rows: 8.0,
            width: 16.0,
            memory_bytes: 0,
            spill_bytes: 0,
        };

        let mut project = PhysicalPlan::new(PhysicalKind::Project);
        project.children = vec![index];

        let mut limit = PhysicalPlan::new(PhysicalKind::Limit);
        limit.children = vec![project];
        limit
    }

    #[test]
    fn trace_record_reports_access_shape_and_limit_pushdown() {
        let plan = index_scan_plan();
        let record = trace_record(&plan);
        assert_eq!(record["trace_version"], 1);
        assert_eq!(record["plan"]["root_kind"], "Limit");
        assert_eq!(record["plan"]["sort_required"], false);
        assert_eq!(record["plan"]["limit_pushdown"], 3);
        assert_eq!(record["chosen_access"]["kind"], "IndexScan");
        assert_eq!(record["chosen_access"]["relation"], "kv");
        assert_eq!(record["chosen_access"]["index"], "kv_tk");
        assert_eq!(record["chosen_access"]["index_probe_kind"], "RangeScan");
        assert_eq!(
            record["chosen_access"]["access_predicates"][0],
            "USING INDEX kv_tk (RangeScan)"
        );
        assert_eq!(record["chosen_access"]["output_order"][0], "k");
        assert_eq!(record["chosen_access"]["covering"], false);
        assert_eq!(record["rejected_paths_complete"], false);
    }

    #[test]
    fn emit_planner_trace_appends_jsonl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let plan = index_scan_plan();
        emit_planner_trace(dir.path(), &plan).expect("emit trace");
        emit_planner_trace(dir.path(), &plan).expect("emit trace");

        let path = dir.path().join(TRACE_FILE);
        let text = std::fs::read_to_string(path).expect("read trace");
        let rows: Vec<_> = text.lines().collect();
        assert_eq!(rows.len(), 2);
        let parsed: serde_json::Value = serde_json::from_str(rows[0]).expect("json row");
        assert_eq!(parsed["chosen_access"]["index"], "kv_tk");
    }
}
