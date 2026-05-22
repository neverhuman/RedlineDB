use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use super::model::{
    CodeShapeScore, DimensionScore, IssueBreakdown, JankuraiComparison, JankuraiRepository,
    do_not_edit, generated_by, generated_command, generated_source,
};

#[derive(Debug)]
struct ParsedScore {
    repo: JankuraiRepository,
    issue_breakdown: Vec<IssueBreakdown>,
}

pub(crate) fn build_comparison(
    redlinedb_score_json: &str,
    sqlite_score_json: &str,
    updated_date: &str,
    sqlite_ref: &str,
) -> Result<JankuraiComparison> {
    let redlinedb = parse_repo_score(redlinedb_score_json, "redlinedb", "RedlineDB", "current")?;
    let sqlite = parse_repo_score(sqlite_score_json, "sqlite", "SQLite", sqlite_ref)?;
    let mut issue_breakdown = Vec::new();
    issue_breakdown.extend(redlinedb.issue_breakdown);
    issue_breakdown.extend(sqlite.issue_breakdown);
    issue_breakdown.sort_by(|left, right| {
        left.repo_id
            .cmp(&right.repo_id)
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.lane.cmp(&right.lane))
    });
    let repositories = vec![redlinedb.repo, sqlite.repo];
    let code_shape = repositories
        .iter()
        .filter_map(code_shape_score)
        .collect::<Vec<_>>();

    Ok(JankuraiComparison {
        generated_by: generated_by(),
        source: generated_source(),
        command: generated_command(),
        do_not_edit_by_hand: do_not_edit(),
        schema_version: "1.0.0".to_owned(),
        updated_date: updated_date.to_owned(),
        sqlite_ref: sqlite_ref.to_owned(),
        repositories,
        issue_breakdown,
        code_shape,
    })
}

pub(crate) fn read_comparison(path: &Path) -> Result<JankuraiComparison> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read jankurai comparison {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parse jankurai comparison {}", path.display()))
}

fn parse_repo_score(
    score_json: &str,
    id: &str,
    label: &str,
    source_ref: &str,
) -> Result<ParsedScore> {
    let value: Value = serde_json::from_str(score_json).context("parse jankurai score JSON")?;
    let score = value
        .get("score")
        .and_then(Value::as_u64)
        .context("jankurai score JSON lacks numeric score")?;
    let raw_score = match value.get("raw_score").and_then(Value::as_u64) {
        Some(raw_score) => raw_score,
        None => score,
    };
    let empty_findings = Vec::new();
    let findings = match value.get("findings").and_then(Value::as_array) {
        Some(findings) => findings.as_slice(),
        None => empty_findings.as_slice(),
    };
    let counted_hard = findings
        .iter()
        .filter(|finding| finding_hardness(finding) == FindingHardness::Hard)
        .count() as u64;
    let counted_soft = findings
        .iter()
        .filter(|finding| finding_hardness(finding) != FindingHardness::Hard)
        .count() as u64;
    let hard_findings = match value
        .pointer("/decision/hard_findings")
        .and_then(Value::as_u64)
    {
        Some(hard_findings) => hard_findings,
        None => counted_hard,
    };
    let soft_findings = match value
        .pointer("/decision/soft_findings")
        .and_then(Value::as_u64)
    {
        Some(soft_findings) => soft_findings,
        None => counted_soft,
    };
    let caps_applied = match value.get("caps_applied").and_then(Value::as_array) {
        Some(caps) => caps.len() as u64,
        None => 0,
    };
    let decision = match value.pointer("/decision/status").and_then(Value::as_str) {
        Some(decision) => decision,
        None => match value.get("conformance_decision").and_then(Value::as_str) {
            Some(decision) => decision,
            None => match value.get("status").and_then(Value::as_str) {
                Some(decision) => decision,
                None => "unknown",
            },
        },
    }
    .trim()
    .to_ascii_lowercase();
    let dimensions = match value.get("dimensions").and_then(Value::as_array) {
        Some(dimensions) => dimensions
            .iter()
            .filter_map(parse_dimension)
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };
    let issue_breakdown = issue_breakdown(id, label, findings);

    Ok(ParsedScore {
        repo: JankuraiRepository {
            id: id.to_owned(),
            label: label.to_owned(),
            source_ref: source_ref.to_owned(),
            score,
            raw_score,
            hard_findings,
            soft_findings,
            caps_applied,
            decision,
            dimensions,
        },
        issue_breakdown,
    })
}

fn parse_dimension(value: &Value) -> Option<DimensionScore> {
    let name = value.get("name")?.as_str()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(DimensionScore {
        name: name.to_owned(),
        score: match value.get("score").and_then(Value::as_f64) {
            Some(score) => score,
            None => 0.0,
        },
        weight: value.get("weight").and_then(Value::as_f64),
        weighted_points: value.get("weighted_points").and_then(Value::as_f64),
    })
}

fn issue_breakdown(repo_id: &str, repo_label: &str, findings: &[Value]) -> Vec<IssueBreakdown> {
    let mut grouped = BTreeMap::<(String, String), (u64, u64)>::new();
    for finding in findings {
        let category = finding
            .get("category")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let category = match category {
            Some(category) => category,
            None => "uncategorized",
        }
        .to_owned();
        let lane = finding
            .get("lane")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let lane = match lane {
            Some(lane) => lane,
            None => "unknown",
        }
        .to_owned();
        let entry = grouped.entry((category, lane)).or_insert((0, 0));
        match finding_hardness(finding) {
            FindingHardness::Hard => entry.0 = entry.0.saturating_add(1),
            FindingHardness::Soft => entry.1 = entry.1.saturating_add(1),
        }
    }
    grouped
        .into_iter()
        .map(
            |((category, lane), (hard_findings, soft_findings))| IssueBreakdown {
                repo_id: repo_id.to_owned(),
                repo_label: repo_label.to_owned(),
                category,
                lane,
                hard_findings,
                soft_findings,
                total_findings: hard_findings.saturating_add(soft_findings),
            },
        )
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FindingHardness {
    Hard,
    Soft,
}

fn finding_hardness(finding: &Value) -> FindingHardness {
    match finding.get("hardness").and_then(Value::as_str) {
        Some(value) if value.eq_ignore_ascii_case("hard") => FindingHardness::Hard,
        _ => FindingHardness::Soft,
    }
}

fn code_shape_score(repo: &JankuraiRepository) -> Option<CodeShapeScore> {
    let dimension = repo
        .dimensions
        .iter()
        .find(|dimension| dimension.name.to_ascii_lowercase().contains("code shape"))?;
    Some(CodeShapeScore {
        repo_id: repo.id.clone(),
        repo_label: repo.label.clone(),
        dimension: dimension.name.clone(),
        score: dimension.score,
        weighted_points: dimension.weighted_points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_handles_missing_optional_fields() {
        let parsed = parse_repo_score(
            r#"{
              "score": 91,
              "findings": [
                { "category": "security" },
                { "hardness": "hard", "lane": "audit" }
              ]
            }"#,
            "sqlite",
            "SQLite",
            "version-3.fixture",
        )
        .expect("parse");

        assert_eq!(parsed.repo.raw_score, 91);
        assert_eq!(parsed.repo.hard_findings, 1);
        assert_eq!(parsed.repo.soft_findings, 1);
        assert_eq!(parsed.repo.caps_applied, 0);
        assert_eq!(parsed.repo.decision, "unknown");
        assert!(parsed.repo.dimensions.is_empty());
        assert!(parsed.issue_breakdown.iter().any(|row| {
            row.category == "security" && row.lane == "unknown" && row.soft_findings == 1
        }));
    }

    #[test]
    fn comparison_uses_code_shape_dimension_when_present() {
        let redline = r#"{
          "score": 64,
          "raw_score": 72,
          "decision": { "status": "advisory", "hard_findings": 2, "soft_findings": 1 },
          "caps_applied": ["a", "b"],
          "dimensions": [{ "name": "Code shape and semantic surface", "score": 80, "weighted_points": 9.6 }]
        }"#;
        let sqlite = r#"{
          "score": 88,
          "raw_score": 90,
          "decision": { "status": "advisory", "hard_findings": 0, "soft_findings": 2 },
          "dimensions": [{ "name": "Code shape and semantic surface", "score": 70 }]
        }"#;
        let comparison = build_comparison(redline, sqlite, "2026-05-21", "version-3.fixture")
            .expect("comparison");

        assert_eq!(comparison.repositories.len(), 2);
        assert_eq!(comparison.code_shape.len(), 2);
        assert_eq!(comparison.repositories[0].caps_applied, 2);
    }
}
