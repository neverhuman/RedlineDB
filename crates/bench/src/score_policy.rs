//! Score-policy validation used by commit hooks and the Jankurai audit lane.

use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{Context, Result};
use serde_json::{Number, Value};

pub fn compare_files(before: &Path, after: &Path) -> Result<Vec<String>> {
    let before = read_document(before)?;
    let after = read_document(after)?;
    compare_documents(&before, &after)
}

pub fn compare_documents(before: &Value, after: &Value) -> Result<Vec<String>> {
    require_object(before, "before score document")?;
    require_object(after, "after score document")?;

    let before_caps = caps(before);
    let after_caps = caps(after);
    let added_caps = after_caps
        .difference(&before_caps)
        .cloned()
        .collect::<Vec<_>>();
    let mut regressions = Vec::new();

    for name in ["score", "raw_score"] {
        if let (Some(old), Some(new)) = (score_metric(before, name), score_metric(after, name))
            && number_value(new) < number_value(old)
        {
            regressions.push(format!(
                "{name} decreased: {} -> {}",
                display_number(old),
                display_number(new)
            ));
        }
    }

    for name in ["hard_findings", "soft_findings"] {
        if let (Some(old), Some(new)) = (integer_metric(before, name), integer_metric(after, name))
            && number_value(new) > number_value(old)
        {
            regressions.push(format!(
                "{name} increased: {} -> {}",
                display_number(old),
                display_number(new)
            ));
        }
    }

    if let (Some(old), Some(new)) = (finding_count(before), finding_count(after))
        && new > old
    {
        regressions.push(format!(
            "finding_count increased: {} -> {}",
            display_integer(old),
            display_integer(new)
        ));
    }

    if after_caps.len() > before_caps.len() {
        regressions.push(format!(
            "applied cap count increased: {} -> {}",
            before_caps.len(),
            after_caps.len()
        ));
    }
    if !added_caps.is_empty() {
        regressions.push(format!("new applied caps: {}", added_caps.join(", ")));
    }

    Ok(regressions)
}

pub fn audit_file_is_acceptable(path: &Path) -> Result<bool> {
    let report = read_document(path)?;
    Ok(audit_report_is_acceptable(&report))
}

pub fn audit_report_is_acceptable(report: &Value) -> bool {
    let Some(report) = report.as_object() else {
        return false;
    };
    let Some(decision) = report.get("decision").and_then(Value::as_object) else {
        return false;
    };
    let Some(ratchet) = decision.get("ratchet").and_then(Value::as_object) else {
        return false;
    };

    let score = report.get("score").and_then(Value::as_f64).unwrap_or(0.0);
    let minimum_score = decision
        .get("minimum_score")
        .and_then(Value::as_f64)
        .unwrap_or(85.0);
    let hard_findings = decision
        .get("hard_findings")
        .and_then(integer_value)
        .unwrap_or(1.0);
    let score_delta = ratchet
        .get("score_delta")
        .and_then(Value::as_f64)
        .unwrap_or(-1.0);

    score >= minimum_score
        && hard_findings == 0.0
        && empty_array_or_missing(report.get("caps_applied"))
        && empty_array_or_missing(ratchet.get("new_caps"))
        && empty_array_or_missing(ratchet.get("new_hard_findings"))
        && score_delta >= 0.0
}

pub fn rejection_message(operation: &str, regressions: &[String]) -> String {
    let mut output = format!("ERROR: score ratchet rejected this {operation}:\n");
    for regression in regressions {
        output.push_str("  - ");
        output.push_str(regression);
        output.push('\n');
    }
    output
}

fn read_document(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse JSON score document {}", path.display()))?;
    require_object(&value, &format!("score document {}", path.display()))?;
    Ok(value)
}

fn require_object<'a>(
    value: &'a Value,
    description: &str,
) -> Result<&'a serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("{description} must be a JSON object"))
}

fn score_metric<'a>(document: &'a Value, name: &str) -> Option<&'a Number> {
    document.get(name)?.as_number()
}

fn integer_metric<'a>(document: &'a Value, name: &str) -> Option<&'a Number> {
    let root_value = document.get(name);
    let value = match root_value {
        None | Some(Value::Null) | Some(Value::Bool(false)) => {
            document.get("decision").and_then(|value| value.get(name))
        }
        value => value,
    }?;
    let number = value.as_number()?;
    integer_value(value).is_some().then_some(number)
}

fn integer_value(value: &Value) -> Option<f64> {
    let number = value.as_f64()?;
    (number.is_finite() && number.fract() == 0.0).then_some(number)
}

fn finding_count(document: &Value) -> Option<f64> {
    if let Some(count) = integer_metric(document, "finding_count") {
        return Some(number_value(count));
    }
    Some(
        number_value(integer_metric(document, "hard_findings")?)
            + number_value(integer_metric(document, "soft_findings")?),
    )
}

fn caps(document: &Value) -> BTreeSet<String> {
    document
        .get("caps_applied")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(jq_string)
        .collect()
}

fn jq_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => display_number(value),
        _ => serde_json::to_string(value).expect("serializing a JSON value cannot fail"),
    }
}

fn empty_array_or_missing(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::Array(values)) => values.is_empty(),
        _ => false,
    }
}

fn number_value(number: &Number) -> f64 {
    number
        .as_f64()
        .expect("serde_json numbers used by score documents fit in f64")
}

fn display_number(number: &Number) -> String {
    let value = number_value(number);
    if value.fract() == 0.0 {
        display_integer(value)
    } else {
        number.to_string()
    }
}

fn display_integer(value: f64) -> String {
    format!("{value:.0}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comparison_reports_every_regression_in_policy_order() {
        let before = json!({
            "score": 98.5,
            "raw_score": 99,
            "decision": {"hard_findings": 0, "soft_findings": 2},
            "caps_applied": ["existing"]
        });
        let after = json!({
            "score": 97.5,
            "raw_score": 98,
            "decision": {"hard_findings": 1, "soft_findings": 3},
            "caps_applied": ["existing", "new-cap"]
        });

        assert_eq!(
            compare_documents(&before, &after).unwrap(),
            vec![
                "score decreased: 98.5 -> 97.5",
                "raw_score decreased: 99 -> 98",
                "hard_findings increased: 0 -> 1",
                "soft_findings increased: 2 -> 3",
                "finding_count increased: 2 -> 4",
                "applied cap count increased: 1 -> 2",
                "new applied caps: new-cap",
            ]
        );
    }

    #[test]
    fn comparison_rejects_non_object_documents() {
        let error = compare_documents(&json!([]), &json!({})).unwrap_err();
        assert!(error.to_string().contains("before score document"));
    }

    #[test]
    fn audit_acceptance_fails_closed_on_malformed_or_regressed_fields() {
        let accepted = json!({
            "score": 96,
            "caps_applied": [],
            "decision": {
                "minimum_score": 95,
                "hard_findings": 0,
                "ratchet": {
                    "new_caps": [],
                    "new_hard_findings": [],
                    "score_delta": 0.5
                }
            }
        });
        assert!(audit_report_is_acceptable(&accepted));

        for rejected in [
            json!([]),
            json!({"score": 100}),
            json!({"score": 96, "decision": {"ratchet": {}}}),
            json!({
                "score": 96,
                "caps_applied": "not-an-array",
                "decision": {
                    "minimum_score": 95,
                    "hard_findings": 0,
                    "ratchet": {"new_caps": [], "new_hard_findings": [], "score_delta": 0}
                }
            }),
        ] {
            assert!(!audit_report_is_acceptable(&rejected));
        }
    }
}
