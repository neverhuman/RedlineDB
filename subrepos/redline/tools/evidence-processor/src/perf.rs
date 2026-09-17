use std::{collections::BTreeSet, fs};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

pub(crate) fn run(args: &[String]) -> Result<()> {
    let [path] = args else {
        bail!("perf-summary: usage: perf-summary JSONL");
    };
    let input = fs::read_to_string(path).with_context(|| format!("perf-summary: read {path}"))?;
    print!("{}", summarize(&input)?);
    Ok(())
}

fn summarize(input: &str) -> Result<String> {
    let mut case_ids = BTreeSet::new();
    let mut ratios = Vec::new();
    for line in input.lines() {
        let row: Value = match serde_json::from_str(line) {
            Ok(row) => row,
            Err(_) => continue,
        };
        let Some(object) = row.as_object() else {
            bail!("perf-summary: JSONL row is not an object");
        };
        let measured = object.get("status").and_then(Value::as_str) == Some("passed")
            && object
                .get("sample_role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.starts_with("measured"))
            && object
                .get("latency_ratio")
                .and_then(Value::as_f64)
                .is_some_and(|ratio| ratio > 0.0);
        if !measured {
            continue;
        }
        let case_id = object
            .get("case_id")
            .and_then(Value::as_str)
            .filter(|case_id| !case_id.is_empty())
            .ok_or_else(|| anyhow!("perf-summary: measured row has no non-empty string case_id"))?;
        case_ids.insert(case_id.to_owned());
        ratios.push(
            object["latency_ratio"]
                .as_f64()
                .expect("measured rows have a positive numeric ratio"),
        );
    }

    ratios.sort_by(f64::total_cmp);
    let mut output = format!(
        "  cases measured: {}\n  samples:        {}\n",
        case_ids.len(),
        ratios.len()
    );
    if !ratios.is_empty() {
        output.push_str(&format!("  ratio median:   {:.3}\n", median(&ratios)));
        if ratios.len() >= 10 {
            output.push_str(&format!(
                "  ratio p90:      {:.3}\n",
                exclusive_decile(&ratios, 9)
            ));
        }
        let faster = ratios.iter().filter(|ratio| **ratio < 1.0).count();
        output.push_str(&format!(
            "  cases faster than sqlite: {faster}/{}\n",
            ratios.len()
        ));
    }
    Ok(output)
}

fn median(sorted: &[f64]) -> f64 {
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

// Matches statistics.quantiles(values, n=10)'s default exclusive method.
fn exclusive_decile(sorted: &[f64], decile: usize) -> f64 {
    debug_assert!(sorted.len() >= 2);
    debug_assert!((1..10).contains(&decile));
    let sample_count = sorted.len();
    let m = sample_count + 1;
    let mut lower = decile * m / 10;
    lower = lower.clamp(1, sample_count - 1);
    let delta = decile as isize * m as isize - lower as isize * 10;
    (sorted[lower - 1] * (10 - delta) as f64 + sorted[lower] * delta as f64) / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_frozen_summary_statistics() {
        let input = (1..=10)
            .map(|index| {
                serde_json::json!({
                    "case_id": format!("case-{index}"),
                    "status": "passed",
                    "sample_role": "measured-1",
                    "latency_ratio": index as f64 / 10.0,
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            summarize(&input).unwrap(),
            "  cases measured: 10\n  samples:        10\n  ratio median:   0.550\n  ratio p90:      0.990\n  cases faster than sqlite: 9/10\n"
        );
    }

    #[test]
    fn skips_malformed_and_unmeasured_rows() {
        let input = "not-json\n{\"status\":\"failed\"}\n";
        assert_eq!(
            summarize(input).unwrap(),
            "  cases measured: 0\n  samples:        0\n"
        );
    }

    #[test]
    fn rejects_measured_rows_without_case_identity() {
        let error =
            summarize(r#"{"status":"passed","sample_role":"measured","latency_ratio":1.2}"#)
                .unwrap_err();
        assert!(error.to_string().contains("string case_id"));
        let error = summarize(
            r#"{"case_id":1,"status":"passed","sample_role":"measured","latency_ratio":1.2}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("string case_id"));
    }
}
