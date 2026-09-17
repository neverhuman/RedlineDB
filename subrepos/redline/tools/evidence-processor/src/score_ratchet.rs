use std::{collections::BTreeSet, fs};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Number, Value};

pub(crate) fn run(args: &[String]) -> Result<()> {
    let [before_path, after_path, context] = args else {
        bail!("score-ratchet: usage: score-ratchet BEFORE_JSON AFTER_JSON CONTEXT");
    };
    let before = read_document(before_path)?;
    let after = read_document(after_path)?;
    let violations = violations(&before, &after)?;
    if !violations.is_empty() {
        bail!(
            "ERROR: score ratchet rejected this {context}:\n  - {}",
            violations.join("\n  - ")
        );
    }
    println!("Score ratchet passed.");
    Ok(())
}

pub(crate) fn run_jankurai(args: &[String]) -> Result<()> {
    let [report_path] = args else {
        bail!("jankurai-ratchet: usage: jankurai-ratchet REPORT_JSON");
    };
    let report = read_document(report_path)?;
    if !jankurai_ratchet_acceptable(&report)? {
        bail!("jankurai-ratchet: report does not satisfy the accepted ratchet state");
    }
    Ok(())
}

fn read_document(path: &str) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("score-ratchet: read {path}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("score-ratchet: parse {path}"))
}

fn violations(before: &Value, after: &Value) -> Result<Vec<String>> {
    let before = before
        .as_object()
        .ok_or_else(|| anyhow!("score-ratchet: baseline document is not an object"))?;
    let after = after
        .as_object()
        .ok_or_else(|| anyhow!("score-ratchet: current document is not an object"))?;
    let mut found = Vec::new();

    for name in ["score", "raw_score"] {
        if let (Some(old), Some(new)) = (numeric_metric(before, name), numeric_metric(after, name))
            && new.value < old.value
        {
            found.push(format!(
                "{name} decreased: {} -> {}",
                old.display, new.display
            ));
        }
    }
    for name in ["hard_findings", "soft_findings"] {
        if let (Some(old), Some(new)) = (integer_metric(before, name), integer_metric(after, name))
            && new.value > old.value
        {
            found.push(format!(
                "{name} increased: {} -> {}",
                old.display, new.display
            ));
        }
    }
    if let (Some(old), Some(new)) = (finding_count(before), finding_count(after))
        && new.value > old.value
    {
        found.push(format!(
            "finding_count increased: {} -> {}",
            old.display, new.display
        ));
    }

    let old_caps = caps(before);
    let new_caps = caps(after);
    if new_caps.len() > old_caps.len() {
        found.push(format!(
            "applied cap count increased: {} -> {}",
            old_caps.len(),
            new_caps.len()
        ));
    }
    let added = new_caps.difference(&old_caps).cloned().collect::<Vec<_>>();
    if !added.is_empty() {
        found.push(format!("new applied caps: {}", added.join(", ")));
    }
    Ok(found)
}

struct NumericMetric {
    value: f64,
    display: String,
}

fn numeric_metric(document: &Map<String, Value>, name: &str) -> Option<NumericMetric> {
    let number = document.get(name)?.as_number()?;
    Some(NumericMetric {
        value: number.as_f64()?,
        display: number.to_string(),
    })
}

struct IntegerMetric {
    value: i128,
    display: String,
}

fn integer_metric(document: &Map<String, Value>, name: &str) -> Option<IntegerMetric> {
    integer_value(document.get(name)).or_else(|| {
        document
            .get("decision")?
            .as_object()
            .and_then(|decision| integer_value(decision.get(name)))
    })
}

fn integer_value(value: Option<&Value>) -> Option<IntegerMetric> {
    let number = value?.as_number()?;
    number_to_i128(number).map(|value| IntegerMetric {
        value,
        display: number.to_string(),
    })
}

fn number_to_i128(number: &Number) -> Option<i128> {
    number
        .as_i64()
        .map(i128::from)
        .or_else(|| number.as_u64().map(i128::from))
}

fn finding_count(document: &Map<String, Value>) -> Option<IntegerMetric> {
    if let Some(explicit) = integer_metric(document, "finding_count") {
        return Some(explicit);
    }
    let hard = integer_metric(document, "hard_findings")?;
    let soft = integer_metric(document, "soft_findings")?;
    let value = hard.value.checked_add(soft.value)?;
    Some(IntegerMetric {
        value,
        display: value.to_string(),
    })
}

fn caps(document: &Map<String, Value>) -> BTreeSet<String> {
    document
        .get("caps_applied")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(python_style_string)
        .collect()
}

fn python_style_string(value: &Value) -> String {
    match value {
        Value::Null => "None".to_owned(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn jankurai_ratchet_acceptable(report: &Value) -> Result<bool> {
    let report = report
        .as_object()
        .ok_or_else(|| anyhow!("jankurai-ratchet: report is not an object"))?;
    let decision = optional_object(report, "decision", "jankurai-ratchet")?;
    let ratchet = optional_object(decision, "ratchet", "jankurai-ratchet decision")?;
    let score = optional_number(report, "score", 0.0, "jankurai-ratchet")?;
    let minimum = optional_number(decision, "minimum_score", 85.0, "jankurai-ratchet decision")?;
    let hard_findings =
        optional_number(decision, "hard_findings", 1.0, "jankurai-ratchet decision")?;
    let score_delta = optional_number(
        ratchet,
        "score_delta",
        -1.0,
        "jankurai-ratchet decision.ratchet",
    )?;
    Ok(score >= minimum
        && hard_findings == 0.0
        && !report.get("caps_applied").is_some_and(json_truthy)
        && !ratchet.get("new_caps").is_some_and(json_truthy)
        && !ratchet.get("new_hard_findings").is_some_and(json_truthy)
        && score_delta >= 0.0)
}

fn optional_object<'a>(
    document: &'a Map<String, Value>,
    name: &str,
    context: &str,
) -> Result<&'a Map<String, Value>> {
    match document.get(name) {
        None => Ok(empty_object()),
        Some(Value::Object(value)) => Ok(value),
        Some(_) => bail!("{context}: {name} is not an object"),
    }
}

fn empty_object() -> &'static Map<String, Value> {
    static EMPTY: std::sync::OnceLock<Map<String, Value>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(Map::new)
}

fn optional_number(
    document: &Map<String, Value>,
    name: &str,
    default: f64,
    context: &str,
) -> Result<f64> {
    match document.get(name) {
        None => Ok(default),
        Some(Value::Number(value)) => value
            .as_f64()
            .ok_or_else(|| anyhow!("{context}: {name} is not a finite number")),
        Some(_) => bail!("{context}: {name} is not a number"),
    }
}

fn json_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_every_regression_in_stable_order() {
        let before = json!({
            "score": 100,
            "raw_score": 99.5,
            "hard_findings": 1,
            "soft_findings": 2,
            "caps_applied": ["old"],
        });
        let after = json!({
            "score": 98,
            "raw_score": 90.25,
            "hard_findings": 2,
            "soft_findings": 3,
            "caps_applied": ["new", "old"],
        });
        assert_eq!(
            violations(&before, &after).unwrap(),
            vec![
                "score decreased: 100 -> 98",
                "raw_score decreased: 99.5 -> 90.25",
                "hard_findings increased: 1 -> 2",
                "soft_findings increased: 2 -> 3",
                "finding_count increased: 3 -> 5",
                "applied cap count increased: 1 -> 2",
                "new applied caps: new",
            ]
        );
    }

    #[test]
    fn accepts_improvement_and_reads_decision_fallback() {
        let before = json!({"score": 90, "decision": {"hard_findings": 2, "soft_findings": 3}});
        let after = json!({"score": 91, "decision": {"hard_findings": 1, "soft_findings": 3}});
        assert!(violations(&before, &after).unwrap().is_empty());
    }

    #[test]
    fn rejects_non_object_documents() {
        let error = violations(&json!([]), &json!({})).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("baseline document is not an object")
        );
    }

    #[test]
    fn validates_jankurai_acceptance_without_fail_open_defaults() {
        let accepted = json!({
            "score": 90,
            "caps_applied": [],
            "decision": {
                "minimum_score": 85,
                "hard_findings": 0,
                "ratchet": {
                    "score_delta": 0,
                    "new_caps": [],
                    "new_hard_findings": [],
                }
            }
        });
        assert!(jankurai_ratchet_acceptable(&accepted).unwrap());

        let missing_hard_findings = json!({
            "score": 90,
            "decision": {"minimum_score": 85, "ratchet": {"score_delta": 0}}
        });
        assert!(!jankurai_ratchet_acceptable(&missing_hard_findings).unwrap());

        let malformed = json!({"score": "90", "decision": {}});
        assert!(
            jankurai_ratchet_acceptable(&malformed)
                .unwrap_err()
                .to_string()
                .contains("score is not a number")
        );
    }
}
