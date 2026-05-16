//! Group records by engine/workload/etc. and build comparison summaries.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::normalize::record_source_paths;

/// Replicates Python's `statistics.median`. For an even number of
/// values, the mean of the two middle elements is returned as an f64.
/// Returns NaN on empty input to match `float("nan")`.
pub(crate) fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Median that preserves Python's `statistics.median` typing semantics
/// for JSON output: when the list has an odd length, the middle JSON
/// value is returned as-is (int stays int, float stays float); when
/// the list has an even length, the average of the two middles is
/// always a float. Returns Null for an empty list.
pub(crate) fn median_value(values: &[Value]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    let mut idx: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (i, v.as_f64().unwrap_or(f64::NAN)))
        .collect();
    idx.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    let n = idx.len();
    if n % 2 == 1 {
        values[idx[n / 2].0].clone()
    } else {
        let lo = idx[n / 2 - 1].1;
        let hi = idx[n / 2].1;
        let avg = (lo + hi) / 2.0;
        serde_json::Number::from_f64(avg)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn collect_pointer(runs: &[&Value], pointer: &str) -> Vec<Value> {
    runs.iter()
        .filter_map(|r| r.pointer(pointer).cloned())
        .collect()
}

pub(crate) fn engine_summary(runs: &[&Value]) -> Value {
    let qps = collect_pointer(runs, "/metrics/throughput_ops_per_sec");
    let p99 = collect_pointer(runs, "/metrics/latency/p99_us");
    let failures = collect_pointer(runs, "/metrics/failures");
    let busy = collect_pointer(runs, "/metrics/busy_errors");
    let locked = collect_pointer(runs, "/metrics/locked_errors");
    let timeout = collect_pointer(runs, "/metrics/timeout_errors");
    let elapsed = collect_pointer(runs, "/metrics/elapsed_ms");
    let raw_paths: Vec<Value> = runs
        .iter()
        .map(|r| {
            r.get("_path")
                .cloned()
                .unwrap_or(Value::String(String::new()))
        })
        .collect();
    let run_ids: Vec<Value> = runs
        .iter()
        .map(|r| r.get("run_id").cloned().unwrap_or(Value::Null))
        .collect();
    json!({
        "runs": runs.len(),
        "median_qps": median_value(&qps),
        "median_p99_us": median_value(&p99),
        "median_failures": median_value(&failures),
        "median_busy_errors": median_value(&busy),
        "median_locked_errors": median_value(&locked),
        "median_timeout_errors": median_value(&timeout),
        "median_elapsed_ms": median_value(&elapsed),
        "raw_record_paths": raw_paths,
        "run_ids": run_ids,
    })
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct ComparisonKey {
    pub workload: String,
    pub durability: String,
    pub threads: i64,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct GroupKey {
    pub engine: String,
    pub workload: String,
    pub durability: String,
    pub threads: i64,
}

pub(crate) fn extract_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => match n.as_i64() {
            Some(v) => Some(v),
            None => n.as_f64().map(|f| f as i64),
        },
        _ => None,
    }
}

pub(crate) fn build_groups<'a>(records: &'a [Value]) -> BTreeMap<GroupKey, Vec<&'a Value>> {
    let mut grouped: BTreeMap<GroupKey, Vec<&Value>> = BTreeMap::new();
    for record in records {
        let run = record.get("_run").and_then(Value::as_str).unwrap_or("");
        if run.ends_with("-w0") {
            continue;
        }
        let key = GroupKey {
            engine: record
                .get("engine")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            workload: record
                .get("workload")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            durability: record
                .get("durability")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            threads: record.get("threads").and_then(extract_int).unwrap_or(0),
        };
        grouped.entry(key).or_default().push(record);
    }
    grouped
}

pub(crate) fn build_comparisons(records: &[Value]) -> Vec<Value> {
    let grouped = build_groups(records);
    let mut by_workload: BTreeMap<ComparisonKey, BTreeMap<String, Vec<&Value>>> = BTreeMap::new();
    for (key, runs) in grouped {
        let comp_key = ComparisonKey {
            workload: key.workload.clone(),
            durability: key.durability.clone(),
            threads: key.threads,
        };
        by_workload
            .entry(comp_key)
            .or_default()
            .insert(key.engine.clone(), runs);
    }
    let mut comparisons = Vec::new();
    for (cmp_key, engines) in by_workload {
        let empty: Vec<&Value> = Vec::new();
        let redline_runs = engines.get("redline").unwrap_or(&empty);
        let sqlite_runs = engines.get("sqlite").unwrap_or(&empty);
        let mut source_paths: Vec<String> = redline_runs
            .iter()
            .chain(sqlite_runs.iter())
            .flat_map(|r| record_source_paths(r))
            .collect();
        source_paths.sort();
        source_paths.dedup();
        let mut comparison = Map::new();
        comparison.insert(
            "workload".to_string(),
            Value::String(cmp_key.workload.clone()),
        );
        comparison.insert(
            "durability".to_string(),
            Value::String(cmp_key.durability.clone()),
        );
        comparison.insert("threads".to_string(), Value::Number(cmp_key.threads.into()));
        comparison.insert(
            "test_code_paths".to_string(),
            Value::Array(source_paths.into_iter().map(Value::String).collect()),
        );
        if !redline_runs.is_empty() {
            comparison.insert("redline".to_string(), engine_summary(redline_runs));
        }
        if !sqlite_runs.is_empty() {
            comparison.insert("sqlite".to_string(), engine_summary(sqlite_runs));
        }
        if !redline_runs.is_empty() && !sqlite_runs.is_empty() {
            let redline_qps = comparison["redline"]["median_qps"].as_f64();
            let sqlite_qps = comparison["sqlite"]["median_qps"].as_f64();
            let ratio = match (redline_qps, sqlite_qps) {
                (Some(r), Some(s)) if s != 0.0 && !s.is_nan() => {
                    json!(r / s)
                }
                _ => Value::Null,
            };
            comparison.insert("ratio_redline_vs_sqlite".to_string(), ratio);
        }
        comparisons.push(Value::Object(comparison));
    }
    comparisons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_matches_python_semantics() {
        assert!(median(vec![]).is_nan());
        assert_eq!(median(vec![1.0]), 1.0);
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        // Even length averages the two middle values, like statistics.median.
        assert_eq!(median(vec![1.0, 2.0, 3.0, 4.0]), 2.5);
    }
}
