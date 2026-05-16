//! Normalization of raw records: source-path inference and field rewriting.

use serde_json::Value;

pub(crate) fn infer_source_paths(workload: &str) -> Vec<String> {
    let chaos = [
        "chaos-lock-convoy",
        "chaos-connection-churn",
        "chaos-checkpoint-thrash",
        "chaos-index-hammer",
        "chaos-sort-spill-convoy",
        "chaos-schema-storm",
    ];
    let index_batch = ["secondary-index-range", "secondary-index-count"];
    let index_access = [
        "secondary-index-read",
        "covered-range-cold",
        "covered-range-warm",
    ];
    let top = ["secondary-index-ordered-limit"];
    let workload_rs = [
        "single-row-insert",
        "batched-insert100",
        "point-read-pk",
        "writers-disjoint",
        "hot-row-update",
        "mixed-oltp",
        "mixed95-read5-write",
        "mixed80-read20-write",
        "mixed50-read50-write",
        "connection-limit",
        "large-sort-spill",
        "json-path-extract",
        "json-path-update",
        "vector-flat-search",
        "vector-ann-search",
        "vector-ann-search-disk",
        "commit-storm-batched",
        "hot-counter-update",
        "queue-mixed",
    ];
    let mut source_paths = Vec::new();
    if chaos.contains(&workload) {
        source_paths.push("crates/bench/src/chaos.rs".to_string());
    } else if index_batch.contains(&workload) {
        source_paths.push("crates/sql/src/exec/index_batch.rs".to_string());
    } else if index_access.contains(&workload) {
        source_paths.push("crates/sql/src/exec/index_access.rs".to_string());
    } else if top.contains(&workload) {
        source_paths.push("crates/sql/src/exec/select_top.rs".to_string());
    } else if workload_rs.contains(&workload) {
        source_paths.push("crates/bench/src/workload.rs".to_string());
    } else {
        source_paths.push("crates/bench/src/workload.rs".to_string());
    }
    source_paths
}

pub(crate) fn record_source_paths(record: &Value) -> Vec<String> {
    if let Some(stats) = record.get("engine_stats").and_then(Value::as_object) {
        if let Some(p) = stats.get("test_code_path").and_then(Value::as_str) {
            if !p.is_empty() {
                return vec![p.to_string()];
            }
        }
    }
    let workload = record.get("workload").and_then(Value::as_str).unwrap_or("");
    infer_source_paths(workload)
}

pub(crate) fn normalize_record(record: &Value, manifest: Option<&Value>) -> Value {
    let mut out = record.clone();
    let workload = record
        .get("workload")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut source_paths: Vec<String> = Vec::new();
    if let Some(stats) = record.get("engine_stats").and_then(Value::as_object) {
        if let Some(p) = stats.get("test_code_path").and_then(Value::as_str) {
            if !p.is_empty() {
                source_paths.push(p.to_string());
            }
        }
    }
    if source_paths.is_empty() {
        source_paths = infer_source_paths(&workload);
    }
    let path = record
        .get("_path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let run = record
        .get("_run")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Value::Object(ref mut map) = out {
        map.insert("raw_path".to_string(), Value::String(path));
        map.insert("run_dir".to_string(), Value::String(run));
        let mut dedup: Vec<String> = source_paths;
        dedup.sort();
        dedup.dedup();
        map.insert(
            "test_code_paths".to_string(),
            Value::Array(dedup.into_iter().map(Value::String).collect()),
        );
        if let Some(m) = manifest {
            if let Some(cp) = m.get("config_path").and_then(Value::as_str) {
                map.insert("config_path".to_string(), Value::String(cp.to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infer_source_paths_routes_workloads_to_owners() {
        assert_eq!(
            infer_source_paths("chaos-lock-convoy"),
            vec!["crates/bench/src/chaos.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-range"),
            vec!["crates/sql/src/exec/index_batch.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-read"),
            vec!["crates/sql/src/exec/index_access.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("secondary-index-ordered-limit"),
            vec!["crates/sql/src/exec/select_top.rs".to_string()]
        );
        assert_eq!(
            infer_source_paths("commit-storm-batched"),
            vec!["crates/bench/src/workload.rs".to_string()]
        );
        // Unknown workload falls through to the workload.rs catch-all.
        assert_eq!(
            infer_source_paths("nonsense-workload"),
            vec!["crates/bench/src/workload.rs".to_string()]
        );
    }

    #[test]
    fn record_source_paths_for_known_workload() {
        // Engine_stats.test_code_path wins when present and non-empty.
        let rec = json!({
            "workload": "chaos-lock-convoy",
            "engine_stats": { "test_code_path": "custom/path.rs" }
        });
        assert_eq!(record_source_paths(&rec), vec!["custom/path.rs".to_string()]);

        // Falls back to infer_source_paths when engine_stats path is empty.
        let rec = json!({
            "workload": "secondary-index-count",
            "engine_stats": { "test_code_path": "" }
        });
        assert_eq!(
            record_source_paths(&rec),
            vec!["crates/sql/src/exec/index_batch.rs".to_string()]
        );

        // Falls back when engine_stats is missing entirely.
        let rec = json!({ "workload": "chaos-index-hammer" });
        assert_eq!(
            record_source_paths(&rec),
            vec!["crates/bench/src/chaos.rs".to_string()]
        );
    }
}
