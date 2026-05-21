use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JankuraiComparison {
    pub(crate) schema_version: String,
    pub(crate) updated_date: String,
    pub(crate) sqlite_ref: String,
    pub(crate) repositories: Vec<JankuraiRepository>,
    pub(crate) issue_breakdown: Vec<IssueBreakdown>,
    pub(crate) code_shape: Vec<CodeShapeScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct JankuraiRepository {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) source_ref: String,
    pub(crate) score: u64,
    pub(crate) raw_score: u64,
    pub(crate) hard_findings: u64,
    pub(crate) soft_findings: u64,
    pub(crate) caps_applied: u64,
    pub(crate) decision: String,
    pub(crate) dimensions: Vec<DimensionScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct DimensionScore {
    pub(crate) name: String,
    pub(crate) score: f64,
    pub(crate) weight: Option<f64>,
    pub(crate) weighted_points: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct IssueBreakdown {
    pub(crate) repo_id: String,
    pub(crate) repo_label: String,
    pub(crate) category: String,
    pub(crate) lane: String,
    pub(crate) hard_findings: u64,
    pub(crate) soft_findings: u64,
    pub(crate) total_findings: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct CodeShapeScore {
    pub(crate) repo_id: String,
    pub(crate) repo_label: String,
    pub(crate) dimension: String,
    pub(crate) score: f64,
    pub(crate) weighted_points: Option<f64>,
}

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

pub(crate) fn comparison_json(comparison: &JankuraiComparison) -> Result<String> {
    Ok(serde_json::to_string_pretty(comparison)? + "\n")
}

pub(crate) fn comparison_csv(comparison: &JankuraiComparison) -> String {
    let mut out = String::from(
        "section,repo,metric,value,category,lane,hard_findings,soft_findings,total_findings\n",
    );
    for repo in &comparison.repositories {
        for (metric, value) in [
            ("score", repo.score),
            ("raw_score", repo.raw_score),
            ("hard_findings", repo.hard_findings),
            ("soft_findings", repo.soft_findings),
            ("caps_applied", repo.caps_applied),
        ] {
            out.push_str(&format!(
                "summary,{},{},{},,,,,\n",
                csv(&repo.label),
                metric,
                value
            ));
        }
    }
    for row in &comparison.issue_breakdown {
        out.push_str(&format!(
            "breakdown,{},{},,{},{},{},{},{}\n",
            csv(&row.repo_label),
            "findings",
            csv(&row.category),
            csv(&row.lane),
            row.hard_findings,
            row.soft_findings,
            row.total_findings
        ));
    }
    for row in &comparison.code_shape {
        out.push_str(&format!(
            "code_shape,{},{},{:.2},,,,,\n",
            csv(&row.repo_label),
            csv(&row.dimension),
            row.score
        ));
    }
    out
}

pub(crate) fn write_or_check(
    comparison: &JankuraiComparison,
    json_path: &Path,
    csv_path: &Path,
    check: bool,
) -> Result<()> {
    let json = comparison_json(comparison)?;
    let csv = comparison_csv(comparison);
    let writes = [(json_path, json), (csv_path, csv)];
    if check {
        let mut drifted = Vec::new();
        for (path, contents) in &writes {
            let current = fs::read_to_string(path).unwrap_or_default();
            if current != *contents {
                drifted.push(path.display().to_string());
            }
        }
        if !drifted.is_empty() {
            bail!("jankurai comparison drift: {}", drifted.join(", "));
        }
        return Ok(());
    }
    for (path, contents) in writes {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
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
    let raw_score = value
        .get("raw_score")
        .and_then(Value::as_u64)
        .unwrap_or(score);
    let findings = value
        .get("findings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let counted_hard = findings
        .iter()
        .filter(|finding| finding_hardness(finding) == FindingHardness::Hard)
        .count() as u64;
    let counted_soft = findings
        .iter()
        .filter(|finding| finding_hardness(finding) != FindingHardness::Hard)
        .count() as u64;
    let hard_findings = value
        .pointer("/decision/hard_findings")
        .and_then(Value::as_u64)
        .unwrap_or(counted_hard);
    let soft_findings = value
        .pointer("/decision/soft_findings")
        .and_then(Value::as_u64)
        .unwrap_or(counted_soft);
    let caps_applied = value
        .get("caps_applied")
        .and_then(Value::as_array)
        .map(|caps| caps.len() as u64)
        .unwrap_or(0);
    let decision = value
        .pointer("/decision/status")
        .and_then(Value::as_str)
        .or_else(|| value.get("conformance_decision").and_then(Value::as_str))
        .or_else(|| value.get("status").and_then(Value::as_str))
        .unwrap_or("unknown")
        .trim()
        .to_ascii_lowercase();
    let dimensions = value
        .get("dimensions")
        .and_then(Value::as_array)
        .map(|dimensions| {
            dimensions
                .iter()
                .filter_map(parse_dimension)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
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
        score: value.get("score").and_then(Value::as_f64).unwrap_or(0.0),
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
            .filter(|value| !value.is_empty())
            .unwrap_or("uncategorized")
            .to_owned();
        let lane = finding
            .get("lane")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("unknown")
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

pub(crate) fn svg(comparison: &JankuraiComparison) -> String {
    let width = 900.0;
    let height = 520.0;
    let left = 150.0;
    let bar_w = 245.0;
    let gap = 34.0;
    let redline_x = left;
    let sqlite_x = left + bar_w + gap;
    let metric_top = 84.0;
    let row_h = 48.0;
    let bar_h = 13.0;
    let repositories = &comparison.repositories;
    let redline = repositories.iter().find(|repo| repo.id == "redlinedb");
    let sqlite = repositories.iter().find(|repo| repo.id == "sqlite");
    let mut out = String::new();
    out.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="jankurai-compare-title jankurai-compare-desc">
<title id="jankurai-compare-title">Jankurai audit comparison, Updated {}</title>
<desc id="jankurai-compare-desc">Comparison of RedlineDB and SQLite Jankurai audit scores, finding counts, applied caps, issue lanes, and code-shape dimensions where present. SQLite ref: {}.</desc>
<rect width="{width}" height="{height}" fill="#ffffff"/>
<text x="28" y="28" font-family="sans-serif" font-size="18" font-weight="700" fill="#111827">Jankurai audit comparison</text>
<text x="28" y="50" font-family="sans-serif" font-size="12" fill="#374151">SQLite source ref: {}; advisory report only, no KPI gate.</text>
<text x="{:.2}" y="76" font-family="sans-serif" font-size="12" font-weight="700" fill="#059669" text-anchor="middle">RedlineDB</text>
<text x="{:.2}" y="76" font-family="sans-serif" font-size="12" font-weight="700" fill="#e11d48" text-anchor="middle">SQLite</text>
"##,
        comparison.updated_date,
        xml(&comparison.sqlite_ref),
        xml(&comparison.sqlite_ref),
        redline_x + bar_w / 2.0,
        sqlite_x + bar_w / 2.0
    ));

    if let (Some(redline), Some(sqlite)) = (redline, sqlite) {
        let metrics = [
            MetricRow::new("score", redline.score, sqlite.score, 100),
            MetricRow::new("raw score", redline.raw_score, sqlite.raw_score, 100),
            MetricRow::new(
                "hard findings",
                redline.hard_findings,
                sqlite.hard_findings,
                redline.hard_findings.max(sqlite.hard_findings).max(1),
            ),
            MetricRow::new(
                "soft findings",
                redline.soft_findings,
                sqlite.soft_findings,
                redline.soft_findings.max(sqlite.soft_findings).max(1),
            ),
            MetricRow::new(
                "caps applied",
                redline.caps_applied,
                sqlite.caps_applied,
                redline.caps_applied.max(sqlite.caps_applied).max(1),
            ),
        ];
        for (index, metric) in metrics.iter().enumerate() {
            let y = metric_top + index as f64 * row_h;
            out.push_str(&format!(
                r##"<text x="28" y="{:.2}" font-family="sans-serif" font-size="12" fill="#111827">{}</text>
"##,
                y + 13.0,
                xml(metric.label)
            ));
            push_metric_bar(
                &mut out,
                redline_x,
                y,
                bar_w,
                bar_h,
                metric.redline,
                metric.scale,
                "#059669",
            );
            push_metric_bar(
                &mut out,
                sqlite_x,
                y,
                bar_w,
                bar_h,
                metric.sqlite,
                metric.scale,
                "#e11d48",
            );
        }
    }

    let issue_y = 342.0;
    out.push_str(
        r##"<text x="28" y="326" font-family="sans-serif" font-size="14" font-weight="700" fill="#111827">Issue lanes</text>
"##,
    );
    for (index, row) in top_issue_rows(comparison).iter().enumerate() {
        let y = issue_y + index as f64 * 22.0;
        out.push_str(&format!(
            r##"<text x="28" y="{y:.2}" font-family="sans-serif" font-size="11" fill="#374151">{} {} / {}: hard {}, soft {}</text>
"##,
            xml(&row.repo_label),
            xml(&row.category),
            xml(&row.lane),
            row.hard_findings,
            row.soft_findings
        ));
    }

    let code_y = 468.0;
    out.push_str(
        r##"<text x="500" y="326" font-family="sans-serif" font-size="14" font-weight="700" fill="#111827">Code shape</text>
"##,
    );
    if comparison.code_shape.is_empty() {
        out.push_str(
            r##"<text x="500" y="350" font-family="sans-serif" font-size="11" fill="#6b7280">dimension unavailable in one or both audits</text>
"##,
        );
    } else {
        for (index, row) in comparison.code_shape.iter().enumerate() {
            let y = 342.0 + index as f64 * 32.0;
            let fill = if row.repo_id == "redlinedb" {
                "#059669"
            } else {
                "#e11d48"
            };
            out.push_str(&format!(
                r##"<text x="500" y="{:.2}" font-family="sans-serif" font-size="11" fill="#374151">{}</text>
"##,
                y + 10.0,
                xml(&row.repo_label)
            ));
            push_metric_bar(
                &mut out,
                590.0,
                y,
                210.0,
                bar_h,
                row.score.round() as u64,
                100,
                fill,
            );
        }
    }
    out.push_str(&format!(
        r##"<text x="28" y="{code_y}" font-family="sans-serif" font-size="10" fill="#6b7280">Metrics come from committed RedlineDB score JSON and a full Jankurai audit of only the cloned SQLite checkout.</text>
</svg>
"##
    ));
    out
}

#[derive(Debug)]
struct MetricRow {
    label: &'static str,
    redline: u64,
    sqlite: u64,
    scale: u64,
}

impl MetricRow {
    fn new(label: &'static str, redline: u64, sqlite: u64, scale: u64) -> Self {
        Self {
            label,
            redline,
            sqlite,
            scale,
        }
    }
}

fn push_metric_bar(
    out: &mut String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    value: u64,
    scale: u64,
    fill: &str,
) {
    let actual_w = value as f64 / scale.max(1) as f64 * width;
    out.push_str(&format!(
        r##"<rect x="{x:.2}" y="{y:.2}" width="{width:.2}" height="{height:.2}" rx="2" fill="#f3f4f6"/>
<rect x="{x:.2}" y="{y:.2}" width="{:.2}" height="{height:.2}" rx="2" fill="{fill}"/>
<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="11" fill="#111827">{value}</text>
"##,
        actual_w.max(1.0),
        x + width + 8.0,
        y + 11.0
    ));
}

fn top_issue_rows(comparison: &JankuraiComparison) -> Vec<IssueBreakdown> {
    let mut rows = comparison.issue_breakdown.clone();
    rows.sort_by(|left, right| {
        right
            .total_findings
            .cmp(&left.total_findings)
            .then_with(|| left.repo_id.cmp(&right.repo_id))
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.lane.cmp(&right.lane))
    });
    rows.into_iter().take(5).collect()
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    #[test]
    fn csv_contains_summary_and_breakdown_rows() {
        let comparison = JankuraiComparison {
            schema_version: "1.0.0".to_owned(),
            updated_date: "2026-05-21".to_owned(),
            sqlite_ref: "version-3.fixture".to_owned(),
            repositories: vec![JankuraiRepository {
                id: "redlinedb".to_owned(),
                label: "RedlineDB".to_owned(),
                source_ref: "current".to_owned(),
                score: 64,
                raw_score: 72,
                hard_findings: 1,
                soft_findings: 2,
                caps_applied: 3,
                decision: "advisory".to_owned(),
                dimensions: Vec::new(),
            }],
            issue_breakdown: vec![IssueBreakdown {
                repo_id: "redlinedb".to_owned(),
                repo_label: "RedlineDB".to_owned(),
                category: "security".to_owned(),
                lane: "fast".to_owned(),
                hard_findings: 1,
                soft_findings: 0,
                total_findings: 1,
            }],
            code_shape: Vec::new(),
        };
        let csv = comparison_csv(&comparison);

        assert!(csv.contains("summary,RedlineDB,score,64"));
        assert!(csv.contains("breakdown,RedlineDB,findings,,security,fast,1,0,1"));
    }

    #[test]
    fn svg_has_title_description_and_light_background() {
        let comparison = build_comparison(
            r#"{ "score": 64, "raw_score": 72, "decision": { "hard_findings": 1, "soft_findings": 1 }, "findings": [] }"#,
            r#"{ "score": 90, "findings": [] }"#,
            "2026-05-21",
            "version-3.fixture",
        )
        .expect("comparison");
        let svg = svg(&comparison);

        assert!(svg.contains("<title id=\"jankurai-compare-title\">"));
        assert!(svg.contains("<desc id=\"jankurai-compare-desc\">"));
        assert!(svg.contains("fill=\"#ffffff\""));
        assert!(svg.contains("fill=\"#111827\""));
    }
}
