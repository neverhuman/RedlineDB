use std::fs;
use std::path::Path;

use anyhow::{Result, bail};

use super::model::JankuraiComparison;
use super::model::{DO_NOT_EDIT, GENERATED_BY, GENERATED_COMMAND, GENERATED_SOURCE};

pub(crate) fn comparison_json(comparison: &JankuraiComparison) -> Result<String> {
    Ok(serde_json::to_string_pretty(comparison)? + "\n")
}

pub(crate) fn comparison_csv(comparison: &JankuraiComparison) -> String {
    let mut out = format!(
        "# {GENERATED_BY}\n# {GENERATED_SOURCE}\n# {GENERATED_COMMAND}\n# {DO_NOT_EDIT}\nsection,repo,metric,value,category,lane,hard_findings,soft_findings,total_findings\n",
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
            let current = match fs::read_to_string(path) {
                Ok(current) => current,
                Err(_) => String::new(),
            };
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
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)?;
    }
    Ok(())
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::{
        IssueBreakdown, JankuraiRepository, do_not_edit, generated_by, generated_command,
        generated_source,
    };
    use super::*;

    #[test]
    fn csv_contains_summary_and_breakdown_rows() {
        let comparison = JankuraiComparison {
            generated_by: generated_by(),
            source: generated_source(),
            command: generated_command(),
            do_not_edit_by_hand: do_not_edit(),
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
}
