use std::path::Path;

use anyhow::{Context, Result, bail};

use super::io::{consume_line_endings, escape_md, improvement_cell};
use super::{
    JANKURAI_BADGE_BEGIN, JANKURAI_BADGE_END, JankuraiScore, LATENCY_TABLE_ANCHOR,
    MIN_FASTER_CASES, MIN_MEDIAN_IMPROVEMENT_PCT, MIN_WORST_IMPROVEMENT_PCT, README_BEGIN,
    README_END, README_JANKURAI_BREAKDOWN_BEGIN, README_JANKURAI_BREAKDOWN_END,
    README_METRICS_BEGIN, README_METRICS_END, RankedCase, SummaryJson,
};

const REDLINE_TESTING_CASES_BASE: &str =
    "https://github.com/neverhuman/redline-testing/blob/main/crates/bench/sqlite_parity/cases";

pub(super) fn readme_block(
    ranked: &[RankedCase],
    summary: &SummaryJson,
    plot: &Path,
    performance_histogram_plot: Option<&Path>,
) -> String {
    let mut out = String::new();
    out.push_str(README_BEGIN);
    out.push('\n');
    out.push_str(&format!(
        "\n**SQLite parity coverage:** **{} / {} = {:.1}%** full generated cases passed in CI. Failed: **{}**. Missing: **{}**. Skipped: **{}**. Updated {}.\n\n",
        summary.passed_cases,
        summary.expected_cases,
        summary.coverage_pct,
        summary.failed_cases,
        summary.missing_cases,
        summary.skipped_cases,
        summary.updated_date
    ));
    out.push_str(
        "Official README metrics and charts are generated only from the pinned external `neverhuman/redline-testing` release artifact, which is the sole official source. The report gate requires `benchmark-results/sqlite-parity/latest/provenance.json` to bind `raw.jsonl` and the `redline-testing` binary sha256 before README/chart regeneration.\n\n",
    );
    out.push_str(&format!(
        "**SQLite parity latency:** median gap **{:.2}%**, worst gap **{:.2}%**, faster cases **{}** with a **{} ns** reference floor (targets: median >= {:.0}%, worst > {:.0}%, faster >= {}).\n\n",
        summary.median_latency_gap_pct,
        summary.worst_latency_gap_pct,
        summary.faster_cases,
        summary.latency_reference_floor_ns,
        MIN_MEDIAN_IMPROVEMENT_PCT,
        MIN_WORST_IMPROVEMENT_PCT,
        MIN_FASTER_CASES
    ));
    out.push_str(&format!(
        "![SQLite parity latency improvement plot]({})\n\n",
        plot.display()
    ));
    if let Some(plot) = performance_histogram_plot {
        out.push_str(&format!(
            "![SQLite parity performance distribution]({})\n\n",
            plot.display()
        ));
    }
    out.push_str(&format!(
        "[Full ranked latency table](#{LATENCY_TABLE_ANCHOR}) is collapsed below for README readability.\n\n"
    ));
    out.push_str(&format!("<details id=\"{LATENCY_TABLE_ANCHOR}\">\n"));
    out.push_str("<summary>Full ranked latency table</summary>\n\n");
    out.push_str("| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |\n");
    out.push_str("| ---: | --- | --- | --- | --- | ---: | ---: | ---: |\n");
    for (index, row) in ranked.iter().enumerate() {
        out.push_str(&format!(
            "| {} | [{} {}]({}/{}) | {} | {} | {} | {} | {} | {} |\n",
            index.saturating_add(1),
            row.case_id,
            escape_md(&row.name),
            REDLINE_TESTING_CASES_BASE,
            row.case_file,
            row.priority,
            row.profile,
            escape_md(&row.category),
            row.sqlite_median_ns,
            row.redline_median_ns,
            improvement_cell(row.improvement_pct)
        ));
    }
    out.push_str("\n</details>\n\n");
    out.push_str(README_END);
    out.push('\n');
    out
}

pub(super) fn metrics_block(
    beyond_sqlite_feature_progress_plot: &Path,
    ksloc_plot: &Path,
    jankurai_score_plot: Option<&Path>,
    code_shape_plot: Option<&Path>,
    median_test_performance_plot: Option<&Path>,
) -> String {
    let mut out = format!(
        "## Engine Metrics\n\n{README_METRICS_BEGIN}\n\n![Beyond-SQLite feature progress chart]({})\n\n",
        beyond_sqlite_feature_progress_plot.display()
    );
    out.push_str(&format!(
        "![SQLite vs RedlineDB production KSLOC chart]({})\n\n",
        ksloc_plot.display()
    ));
    if let Some(plot) = jankurai_score_plot {
        out.push_str(&format!(
            "![RedlineDB vs SQLite Jankurai score chart]({})\n\n",
            plot.display()
        ));
    }
    if let Some(plot) = code_shape_plot {
        out.push_str(&format!(
            "![RedlineDB vs SQLite code shape score chart]({})\n\n",
            plot.display()
        ));
    }
    if let Some(plot) = median_test_performance_plot {
        out.push_str(&format!(
            "![RedlineDB vs SQLite median test performance chart]({})\n\n",
            plot.display()
        ));
    }
    out.push_str(README_METRICS_END);
    out.push('\n');
    out
}

pub(super) fn jankurai_breakdown_block(jankurai_comparison_plot: &Path) -> String {
    format!(
        "## Jankurai Breakdown\n\n{README_JANKURAI_BREAKDOWN_BEGIN}\n\n![RedlineDB vs SQLite Jankurai audit breakdown]({})\n\n{README_JANKURAI_BREAKDOWN_END}\n",
        jankurai_comparison_plot.display()
    )
}

pub(super) fn replace_metrics_block(readme: &str, block: &str) -> Result<String> {
    if let (Some(begin), Some(end)) = (
        readme.find(README_METRICS_BEGIN),
        readme.find(README_METRICS_END),
    ) {
        let mut begin = begin;
        let mut end = end + README_METRICS_END.len();
        if let Some(heading_start) = readme[..begin].rfind("## Engine Metrics") {
            begin = heading_start;
        } else if let Some(heading_start) = readme[..begin].rfind("## Metrics") {
            begin = heading_start;
        }
        end = consume_line_endings(readme, end);
        let mut next = String::new();
        next.push_str(&readme[..begin]);
        next.push_str(block.trim_end());
        next.push_str("\n\n");
        next.push_str(&readme[end..]);
        return Ok(next);
    }

    let intro = "group-commit WAL, and crash recovery designed for multi-writer workloads.\n";
    let Some(index) = readme.find(intro) else {
        bail!("README lacks introductory paragraph for SQLite parity metrics block");
    };
    let insert = index + intro.len();
    let mut next = String::new();
    next.push_str(&readme[..insert]);
    next.push('\n');
    next.push_str(block);
    next.push('\n');
    next.push_str(&readme[insert..]);
    Ok(next)
}

pub(super) fn replace_jankurai_breakdown_block(readme: &str, block: &str) -> Result<String> {
    if let (Some(begin), Some(end)) = (
        readme.find(README_JANKURAI_BREAKDOWN_BEGIN),
        readme.find(README_JANKURAI_BREAKDOWN_END),
    ) {
        let mut begin = begin;
        let mut end = end + README_JANKURAI_BREAKDOWN_END.len();
        if let Some(heading_start) = readme[..begin].rfind("## Jankurai Breakdown") {
            begin = heading_start;
        }
        end = consume_line_endings(readme, end);
        let mut next = String::new();
        next.push_str(&readme[..begin]);
        next.push_str(block.trim_end());
        next.push_str("\n\n");
        next.push_str(&readme[end..]);
        return Ok(next);
    }

    let heading = "## Architecture\n";
    let Some(index) = readme.find(heading) else {
        bail!("README lacks Architecture heading for Jankurai breakdown block");
    };
    let mut next = String::new();
    next.push_str(&readme[..index]);
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(block.trim_end());
    next.push_str("\n\n");
    next.push_str(&readme[index..]);
    Ok(next)
}

pub(super) fn replace_readme_block(readme: &str, block: &str) -> Result<String> {
    if let (Some(begin), Some(end)) = (readme.find(README_BEGIN), readme.find(README_END)) {
        let mut begin = begin;
        let mut end = end + README_END.len();
        let wrapper_prefix = "<details>\n<summary>Detailed parity report</summary>\n\n";
        if readme[..begin].ends_with(wrapper_prefix) {
            begin -= wrapper_prefix.len();
            if readme[end..].starts_with("\n\n</details>") {
                end += "\n\n</details>".len();
            } else if readme[end..].starts_with("\n</details>") {
                end += "\n</details>".len();
            }
        }
        let mut next = String::new();
        next.push_str(&readme[..begin]);
        next.push_str(block.trim_end());
        next.push_str(&readme[end..]);
        return Ok(next);
    }
    let heading = "## SQLite parity test coverage\n";
    let Some(index) = readme.find(heading) else {
        bail!("README lacks SQLite parity test coverage heading and report markers");
    };
    let insert = index + heading.len();
    let mut next = String::new();
    next.push_str(&readme[..insert]);
    next.push('\n');
    next.push_str(block);
    next.push('\n');
    next.push_str(&readme[insert..]);
    Ok(next)
}

pub(super) fn parse_jankurai_score(score_json: &str) -> Result<JankuraiScore> {
    let value: serde_json::Value =
        serde_json::from_str(score_json).context("parse repo-score JSON")?;
    let score = value
        .get("score")
        .and_then(serde_json::Value::as_u64)
        .context("repo-score JSON lacks numeric score")?;
    let status = match (
        value
            .pointer("/decision/status")
            .and_then(serde_json::Value::as_str),
        value.get("decision").and_then(serde_json::Value::as_str),
        value
            .get("conformance_decision")
            .and_then(serde_json::Value::as_str),
        value.get("status").and_then(serde_json::Value::as_str),
    ) {
        (Some(status), _, _, _)
        | (None, Some(status), _, _)
        | (None, None, Some(status), _)
        | (None, None, None, Some(status)) => status,
        _ => bail!("repo-score JSON lacks decision status"),
    };
    let status = status.trim();
    if status.is_empty() {
        bail!("repo-score JSON lacks decision status");
    }
    let status = status.to_ascii_lowercase();
    let color = jankurai_badge_color(score, &status);
    Ok(JankuraiScore {
        score,
        status,
        color,
    })
}

fn jankurai_badge_color(score: u64, status: &str) -> &'static str {
    match status {
        "block" | "blocked" | "fail" | "failed" => "red",
        "pass" | "passed" if score >= 85 => "brightgreen",
        _ if score >= 85 => "green",
        _ if score >= 70 => "yellow",
        _ if score >= 50 => "orange",
        _ => "red",
    }
}

pub(super) fn replace_jankurai_badge(readme: &str, score: &JankuraiScore) -> Result<String> {
    let block = jankurai_badge_block(score);
    if let Some((begin, end)) =
        marked_block_bounds(readme, JANKURAI_BADGE_BEGIN, JANKURAI_BADGE_END)
    {
        if marked_block_is_in_badge_paragraph(readme, begin, end) {
            return Ok(replace_span(readme, begin, end, &block));
        }
        let readme = replace_span(readme, begin, end, "");
        return insert_jankurai_badge(&readme, &block);
    }

    insert_jankurai_badge(readme, &block)
}

pub(super) fn replace_parity_badges(readme: &str, summary: &SummaryJson) -> String {
    let mut out = String::with_capacity(readme.len() + 160);
    let mut inserted = false;
    for line in readme.lines() {
        if line.contains("img.shields.io/badge/approved%20CI")
            || line.contains("img.shields.io/badge/accounted%20parity")
            || line.contains("img.shields.io/badge/full%20corpus")
            || line.contains("img.shields.io/badge/generated%20cases")
        {
            if !inserted {
                out.push_str(&parity_badge_block(summary));
                inserted = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if readme.ends_with('\n') {
        out
    } else {
        out.trim_end_matches('\n').to_owned()
    }
}

fn parity_badge_block(summary: &SummaryJson) -> String {
    let coverage_color = if summary.failed_cases == 0
        && summary.missing_cases == 0
        && summary.skipped_cases == 0
        && summary.passed_cases == summary.expected_cases
    {
        "brightgreen"
    } else {
        "yellow"
    };
    format!(
        "  <a href=\"#sqlite-parity-status\"><img src=\"https://img.shields.io/badge/full%20corpus-{}%2F{}-{coverage_color}\" alt=\"full corpus parity\"></a>\n  <a href=\"#sqlite-parity-status\"><img src=\"https://img.shields.io/badge/generated%20cases-{}-blue\" alt=\"generated cases\"></a>\n",
        summary.passed_cases, summary.expected_cases, summary.generated_cases
    )
}

fn insert_jankurai_badge(readme: &str, block: &str) -> Result<String> {
    let paragraph_end = badge_paragraph_end(readme)?;
    let mut next = String::new();
    next.push_str(&readme[..paragraph_end]);
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&block);
    next.push('\n');
    next.push_str(&readme[paragraph_end..]);
    Ok(next)
}

fn marked_block_bounds(
    readme: &str,
    begin_marker: &str,
    end_marker: &str,
) -> Option<(usize, usize)> {
    let marker_begin = readme.find(begin_marker)?;
    let marker_end = readme.find(end_marker)? + end_marker.len();
    let begin = readme[..marker_begin]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let end = readme[marker_end..]
        .find('\n')
        .map(|index| marker_end + index + 1)
        .unwrap_or(marker_end);
    Some((begin, end))
}

fn replace_span(readme: &str, begin: usize, end: usize, block: &str) -> String {
    let removed_trailing_newline = end > begin && readme.as_bytes().get(end - 1) == Some(&b'\n');
    let mut next = String::new();
    next.push_str(&readme[..begin]);
    if !block.is_empty() {
        next.push_str(block.trim_end());
        if removed_trailing_newline {
            next.push('\n');
        }
    }
    next.push_str(&readme[end..]);
    next
}

fn marked_block_is_in_badge_paragraph(readme: &str, begin: usize, end: usize) -> bool {
    let Some(paragraph_start) = readme[..begin].rfind("<p align=\"center\">") else {
        return false;
    };
    let Some(paragraph_end_offset) = readme[end..].find("</p>") else {
        return false;
    };
    let paragraph_end = end + paragraph_end_offset;
    is_badge_paragraph(&readme[paragraph_start..paragraph_end])
}

fn badge_paragraph_end(readme: &str) -> Result<usize> {
    let paragraph_marker = "<p align=\"center\">";
    let mut search_start = 0;
    while let Some(start_offset) = readme[search_start..].find(paragraph_marker) {
        let paragraph_start = search_start + start_offset;
        let Some(end_offset) = readme[paragraph_start..].find("</p>") else {
            bail!("README top badge paragraph is missing closing </p>");
        };
        let paragraph_end = paragraph_start + end_offset;
        if is_badge_paragraph(&readme[paragraph_start..paragraph_end]) {
            return Ok(paragraph_end);
        }
        search_start = paragraph_end + "</p>".len();
    }
    bail!("README lacks top badge paragraph for jankurai score badge");
}

fn is_badge_paragraph(paragraph: &str) -> bool {
    paragraph.contains("img.shields.io/badge/approved%20CI")
        || paragraph.contains("img.shields.io/badge/version-")
}

pub(super) fn jankurai_badge_block(score: &JankuraiScore) -> String {
    let message = format!("{}/100 {}", score.score, score.status);
    format!(
        "  {JANKURAI_BADGE_BEGIN}\n  <a href=\".jankurai/repo-score.md\"><img src=\"https://img.shields.io/badge/jankurai-{}-{}\" alt=\"jankurai score: {}\"></a>\n  {JANKURAI_BADGE_END}",
        shields_segment(&message),
        score.color,
        message
    )
}

fn shields_segment(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '-' => out.push_str("--"),
            '_' => out.push_str("__"),
            ' ' => out.push_str("%20"),
            '/' => out.push_str("%2F"),
            '%' => out.push_str("%25"),
            '?' => out.push_str("%3F"),
            '#' => out.push_str("%23"),
            '&' => out.push_str("%26"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push_str("%22"),
            '\'' => out.push_str("%27"),
            ch if ch.is_ascii_alphanumeric() || ch == '.' => out.push(ch),
            ch => out.push_str(&format!("%{:02X}", ch as u32)),
        }
    }
    out
}
