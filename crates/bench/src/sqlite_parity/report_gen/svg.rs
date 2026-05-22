use anyhow::{Result, bail};

use super::super::jankurai_compare::JankuraiComparison;
use super::super::source_lines;
use super::super::text::escape_xml;
use super::io::{REPORT_REGEN_COMMAND, color, generated_xml_header};
use super::{RankedCase, SummaryJson};

pub(super) fn latency_svg(ranked: &[RankedCase], summary: &SummaryJson) -> String {
    let width = 1200.0;
    let height = 520.0;
    let left = 70.0;
    let right = 40.0;
    let top = 86.0;
    let bottom = 72.0;
    let values = ranked
        .iter()
        .map(|row| row.improvement_pct)
        .chain(std::iter::once(0.0))
        .collect::<Vec<_>>();
    let min = values.iter().copied().fold(0.0_f64, f64::min).floor();
    let max = values.iter().copied().fold(0.0_f64, f64::max).ceil();
    let span = (max - min).max(1.0);
    let plot_w = width - left - right;
    let plot_h = height - top - bottom;
    let x_for = |index: usize| {
        if ranked.len() <= 1 {
            left + plot_w / 2.0
        } else {
            left + index as f64 / (ranked.len() - 1) as f64 * plot_w
        }
    };
    let y_for = |value: f64| top + (max - value) / span * plot_h;
    let zero_y = y_for(0.0);
    let mut out = generated_xml_header(
        "benchmark-results/sqlite-parity/latest/raw.jsonl",
        REPORT_REGEN_COMMAND,
    );
    out.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">SQLite parity latency gap, Updated {}</title>
<desc id="desc">Floor-adjusted median latency improvement vs SQLite. Positive means RedlineDB is faster; negative means regression. Coverage {} of {} full generated cases with {} measured samples.</desc>
<rect width="1200" height="520" fill="#ffffff"/>
<text x="70" y="34" font-family="sans-serif" font-size="22" font-weight="700">SQLite parity latency improvement vs SQLite, Updated {}</text>
<text x="70" y="60" font-family="sans-serif" font-size="14" fill="#374151">Coverage: {} / {} = {:.1}% full generated cases; measured samples: {}; colormap legend: regression red, near-parity neutral, gain green/blue</text>
<line x1="{left}" y1="{zero_y:.2}" x2="1160" y2="{zero_y:.2}" stroke="#111827" stroke-width="2"/>
<text x="74" y="{:.2}" font-family="sans-serif" font-size="12" fill="#111827">0% horizontal reference line</text>
<line x1="{left}" y1="{top}" x2="{left}" y2="448" stroke="#4b5563"/>
<line x1="{left}" y1="448" x2="1160" y2="448" stroke="#4b5563"/>
<text x="570" y="498" font-family="sans-serif" font-size="13" text-anchor="middle">Ranked full-corpus tests, worst RedlineDB gap to largest gain</text>
<text x="18" y="270" font-family="sans-serif" font-size="13" transform="rotate(-90 18 270)" text-anchor="middle">Floor-adjusted latency improvement vs SQLite (%)</text>
"##,
        summary.updated_date,
        summary.passed_cases,
        summary.expected_cases,
        summary.measured_samples,
        summary.updated_date,
        summary.passed_cases,
        summary.expected_cases,
        summary.coverage_pct,
        summary.measured_samples,
        zero_y - 8.0
    ));
    if !ranked.is_empty() {
        let points = ranked
            .iter()
            .enumerate()
            .map(|(index, row)| format!("{:.2},{:.2}", x_for(index), y_for(row.improvement_pct)))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            r##"<polyline points="{points}" fill="none" stroke="#2563eb" stroke-width="1.5" opacity="0.55"/>
"##
        ));
        for (index, row) in ranked.iter().enumerate() {
            out.push_str(&format!(
                r##"<circle cx="{:.2}" cy="{:.2}" r="3" fill="{}"><title>{} {} {:.2}%</title></circle>
"##,
                x_for(index),
                y_for(row.improvement_pct),
                color(row.improvement_pct),
                row.case_id,
                escape_xml(&row.name),
                row.improvement_pct
            ));
        }
        for (label, index) in [
            ("worst", 0usize),
            ("median", ranked.len() / 2),
            ("best", ranked.len() - 1),
        ] {
            let row = &ranked[index];
            out.push_str(&format!(
                r##"<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="12" fill="#111827">{label}: {} {:.1}%</text>
"##,
                x_for(index).min(980.0),
                (y_for(row.improvement_pct) - 10.0).max(82.0),
                row.case_id,
                row.improvement_pct
            ));
        }
    }
    out.push_str("</svg>\n");
    out
}

pub(super) fn ksloc_svg(summary: &source_lines::SourceLineSummary, updated_date: &str) -> String {
    let sqlite = summary.sqlite_reference_ksloc();
    let redline = summary.redlinedb_ksloc();
    two_bar_svg(&TwoBarChart {
        title_id: "ksloc-title",
        desc_id: "ksloc-desc",
        title: "Production source footprint",
        subtitle: "Core RedlineDB crates scanned without tests, blank lines, or comments",
        desc: "Production Rust source lines in RedlineDB core crates compared with a fixed SQLite source-line reference.",
        source: "crates/{redlinedb,sql,kernel,ffi}/src",
        axis_label: "KSLOC",
        unit: TwoBarUnit::Ksloc,
        scale_max: Some((sqlite.max(redline) / 20.0).ceil() * 20.0),
        redline_value: redline,
        sqlite_value: sqlite,
        lower_is_better: true,
        updated_date,
    })
}

pub(super) fn jankurai_score_svg(
    comparison: &JankuraiComparison,
    updated_date: &str,
) -> Option<String> {
    let redline = repository_value(comparison, "redlinedb", |repo| repo.score as f64)?;
    let sqlite = repository_value(comparison, "sqlite", |repo| repo.score as f64)?;
    Some(two_bar_svg(&TwoBarChart {
        title_id: "jankurai-score-title",
        desc_id: "jankurai-score-desc",
        title: "Jankurai score",
        subtitle: "Advisory audit score from committed RedlineDB and SQLite checkout reports",
        desc: "Jankurai audit score comparison on a 0 to 100 scale.",
        source: "benchmark-results/sqlite-parity/latest/jankurai-comparison.json",
        axis_label: "score",
        unit: TwoBarUnit::Score,
        scale_max: Some(100.0),
        redline_value: redline,
        sqlite_value: sqlite,
        lower_is_better: false,
        updated_date,
    }))
}

pub(super) fn code_shape_svg(
    comparison: &JankuraiComparison,
    updated_date: &str,
) -> Option<String> {
    let redline = code_shape_value(comparison, "redlinedb")?;
    let sqlite = code_shape_value(comparison, "sqlite")?;
    Some(two_bar_svg(&TwoBarChart {
        title_id: "code-shape-title",
        desc_id: "code-shape-desc",
        title: "Code shape score",
        subtitle: "Jankurai code-shape dimension; higher means smaller repair surface",
        desc: "Jankurai code-shape dimension score comparison on a 0 to 100 scale.",
        source: "benchmark-results/sqlite-parity/latest/jankurai-comparison.json",
        axis_label: "score",
        unit: TwoBarUnit::Score,
        scale_max: Some(100.0),
        redline_value: redline,
        sqlite_value: sqlite,
        lower_is_better: false,
        updated_date,
    }))
}

pub(super) fn median_test_performance_svg(summary: &SummaryJson) -> String {
    two_bar_svg(&TwoBarChart {
        title_id: "median-test-performance-title",
        desc_id: "median-test-performance-desc",
        title: "Median test performance",
        subtitle: "Median of per-case medians from ranked.csv; lower is better",
        desc: "Median raw runtime per SQLite parity case, using per-case medians for SQLite and RedlineDB.",
        source: "benchmark-results/sqlite-parity/latest/ranked.csv",
        axis_label: "ms",
        unit: TwoBarUnit::Milliseconds,
        scale_max: None,
        redline_value: ns_to_ms(summary.redline_case_median_ns),
        sqlite_value: ns_to_ms(summary.sqlite_case_median_ns),
        lower_is_better: true,
        updated_date: &summary.updated_date,
    })
}

pub(super) fn beyond_sqlite_feature_progress_svg(
    backlog: &str,
    updated_date: &str,
) -> Result<String> {
    let progress = parse_beyond_sqlite_progress(backlog)?;
    let width = 980.0;
    let height = 252.0;
    let left = 110.0;
    let top = 84.0;
    let bar_h = 28.0;
    let gap = 6.0;
    let segment_w = 54.0;
    let mut out = generated_xml_header("docs/beyond-sqlite-gaps.md", REPORT_REGEN_COMMAND);
    out.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
<title id="title">Beyond-SQLite feature progress, Updated {updated_date}</title>
<desc id="desc">Passing reference backlog areas out of 12 ranked beyond-SQLite feature areas. Green marks passing reference rows; gray marks manifest backlog rows.</desc>
<rect width="{width}" height="{height}" fill="#0f172a"/>
<rect x="16" y="16" width="948" height="220" rx="18" fill="#111827" stroke="#334155"/>
<text x="36" y="52" font-family="sans-serif" font-size="22" font-weight="700" fill="#f8fafc">Beyond-SQLite feature progress</text>
<text x="36" y="74" font-family="sans-serif" font-size="12" fill="#cbd5e1">Passing reference backlog areas out of 12 ranked feature areas, updated {updated_date}</text>
<text x="844" y="56" font-family="sans-serif" font-size="28" font-weight="700" text-anchor="end" fill="#f8fafc">{} / {}</text>
<text x="844" y="74" font-family="sans-serif" font-size="12" text-anchor="end" fill="#cbd5e1">passing reference</text>
"##,
        progress.passed,
        progress.total,
    ));
    out.push_str(&format!(
        r##"<rect x="{left}" y="{top}" width="{}" height="{bar_h}" rx="10" fill="#1e293b"/>
"##,
        (segment_w + gap) * 12.0 - gap
    ));
    for (index, status) in progress.statuses.iter().enumerate() {
        let x = left + index as f64 * (segment_w + gap);
        let fill = if *status == BeyondSqliteStatus::PassingReference {
            "#22c55e"
        } else {
            "#64748b"
        };
        out.push_str(&format!(
            r##"<rect x="{x:.2}" y="{top}" width="{segment_w}" height="{bar_h}" rx="6" fill="{fill}"/>
<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="12" font-weight="700" text-anchor="middle" fill="#0f172a">{}</text>
"##,
            x + segment_w / 2.0,
            top + 19.0,
            index + 1
        ));
    }
    out.push_str(&format!(
        r##"<text x="{left}" y="146" font-family="sans-serif" font-size="14" fill="#e2e8f0">Legend</text>
<rect x="{left}" y="160" width="16" height="16" rx="4" fill="#22c55e"/>
<text x="{:.2}" y="173" font-family="sans-serif" font-size="12" fill="#cbd5e1">Passing reference</text>
<rect x="{left}" y="182" width="16" height="16" rx="4" fill="#64748b"/>
<text x="{:.2}" y="195" font-family="sans-serif" font-size="12" fill="#cbd5e1">Manifest backlog</text>
</svg>
"##,
        left + 24.0,
        left + 24.0
    ));
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BeyondSqliteStatus {
    PassingReference,
    ManifestBacklog,
}

#[derive(Debug)]
struct BeyondSqliteProgress {
    passed: usize,
    total: usize,
    statuses: Vec<BeyondSqliteStatus>,
}

fn parse_beyond_sqlite_progress(backlog: &str) -> Result<BeyondSqliteProgress> {
    let mut statuses = Vec::new();
    let mut passed = 0usize;
    for line in backlog.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.len() < 6 {
            continue;
        }
        if cells[0].parse::<usize>().is_err() {
            continue;
        }
        let status = match cells[5] {
            "Passing reference" => BeyondSqliteStatus::PassingReference,
            "Manifest backlog" => BeyondSqliteStatus::ManifestBacklog,
            other => bail!("unexpected beyond-SQLite backlog status: {other}"),
        };
        if status == BeyondSqliteStatus::PassingReference {
            passed += 1;
        }
        statuses.push(status);
    }
    if statuses.len() != 12 {
        bail!(
            "unexpected beyond-SQLite backlog rank count: {}",
            statuses.len()
        );
    }
    Ok(BeyondSqliteProgress {
        passed,
        total: statuses.len(),
        statuses,
    })
}

struct TwoBarChart<'a> {
    title_id: &'a str,
    desc_id: &'a str,
    title: &'a str,
    subtitle: &'a str,
    desc: &'a str,
    source: &'a str,
    axis_label: &'a str,
    unit: TwoBarUnit,
    scale_max: Option<f64>,
    redline_value: f64,
    sqlite_value: f64,
    lower_is_better: bool,
    updated_date: &'a str,
}

#[derive(Clone, Copy)]
enum TwoBarUnit {
    Ksloc,
    Score,
    Milliseconds,
}

fn two_bar_svg(chart: &TwoBarChart<'_>) -> String {
    let width = 760.0;
    let height = 168.0;
    let left = 132.0;
    let right = 92.0;
    let top = 48.0;
    let bar_h = 24.0;
    let row_gap = 20.0;
    let axis_y = 138.0;
    let plot_w = width - left - right;
    let max = chart
        .scale_max
        .unwrap_or_else(|| nice_axis_max(chart.redline_value.max(chart.sqlite_value)));
    let x_for = |value: f64| left + value / max.max(1.0) * plot_w;
    let bar_w = |value: f64| (x_for(value) - left).max(1.0);
    let redline_y = top;
    let sqlite_y = top + bar_h + row_gap;
    let mut out = generated_xml_header(chart.source, REPORT_REGEN_COMMAND);
    let better_note = if chart.lower_is_better {
        "; lower is better"
    } else {
        "; higher is better"
    };
    out.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="{} {}">
<title id="{}">SQLite vs RedlineDB {}, Updated {}</title>
<desc id="{}">{} RedlineDB: {}; SQLite: {}{}.</desc>
<text x="{left}" y="22" font-family="sans-serif" font-size="17" font-weight="700" fill="#f97316">{}</text>
<text x="{left}" y="39" font-family="sans-serif" font-size="12" fill="#fbbf24">{}; updated {}</text>
"##,
        chart.title_id,
        chart.desc_id,
        chart.title_id,
        escape_xml(chart.title),
        chart.updated_date,
        chart.desc_id,
        escape_xml(chart.desc),
        format_value(chart.redline_value, chart.unit),
        format_value(chart.sqlite_value, chart.unit),
        better_note,
        escape_xml(chart.title),
        escape_xml(chart.subtitle),
        chart.updated_date
    ));
    for value in grid_values(max) {
        let x = x_for(value);
        out.push_str(&format!(
            r##"<line x1="{x:.2}" y1="44" x2="{x:.2}" y2="{axis_y}" stroke="#f59e0b" opacity="0.35"/>
<text x="{x:.2}" y="156" font-family="sans-serif" font-size="10" fill="#fbbf24" text-anchor="middle">{}</text>
"##,
            axis_label_value(value, chart.unit)
        ));
    }
    out.push_str(&format!(
        r##"<line x1="{left}" y1="{axis_y}" x2="{:.2}" y2="{axis_y}" stroke="#fbbf24"/>
<text x="{:.2}" y="156" font-family="sans-serif" font-size="10" fill="#fbbf24" text-anchor="end">{}</text>
"##,
        left + plot_w,
        left + plot_w + 54.0,
        escape_xml(chart.axis_label)
    ));
    push_two_bar_row(
        &mut out,
        "RedlineDB",
        left,
        redline_y,
        bar_h,
        bar_w(chart.redline_value),
        x_for(chart.redline_value),
        "#10b981",
        &format_value(chart.redline_value, chart.unit),
    );
    push_two_bar_row(
        &mut out,
        "SQLite",
        left,
        sqlite_y,
        bar_h,
        bar_w(chart.sqlite_value),
        x_for(chart.sqlite_value),
        "#e11d48",
        &format_value(chart.sqlite_value, chart.unit),
    );
    out.push_str("</svg>\n");
    out
}

fn push_two_bar_row(
    out: &mut String,
    label: &str,
    left: f64,
    y: f64,
    bar_h: f64,
    bar_width: f64,
    bar_end: f64,
    fill: &str,
    value: &str,
) {
    let value_x = bar_end + 8.0;
    let label_inside = value_x > 650.0;
    let text_x = if label_inside {
        (bar_end - 8.0).max(left + 78.0)
    } else {
        value_x
    };
    let text_fill = if label_inside { "#ffffff" } else { "#fbbf24" };
    let text_anchor = if label_inside { "end" } else { "start" };
    out.push_str(&format!(
        r##"<text x="20" y="{:.2}" font-family="sans-serif" font-size="13" fill="#f97316">{}</text>
<rect x="{left}" y="{y}" width="{bar_width:.2}" height="{bar_h}" rx="3" fill="{fill}"/>
<text x="{text_x:.2}" y="{:.2}" font-family="sans-serif" font-size="12" font-weight="700" fill="{text_fill}" text-anchor="{text_anchor}">{}</text>
"##,
        y + 16.5,
        escape_xml(label),
        y + 16.5,
        escape_xml(value)
    ));
}

fn grid_values(max: f64) -> Vec<f64> {
    let step = nice_axis_step(max);
    let mut values = Vec::new();
    let mut value = 0.0;
    while value <= max + f64::EPSILON {
        values.push(value);
        value += step;
    }
    values
}

fn nice_axis_max(value: f64) -> f64 {
    if value <= 0.0 {
        return 1.0;
    }
    let step = nice_axis_step(value);
    (value / step).ceil() * step
}

fn nice_axis_step(max: f64) -> f64 {
    let rough = (max / 4.0).max(1.0);
    let magnitude = 10_f64.powf(rough.log10().floor());
    let normalized = rough / magnitude;
    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

fn axis_label_value(value: f64, unit: TwoBarUnit) -> String {
    match unit {
        TwoBarUnit::Ksloc | TwoBarUnit::Milliseconds => format!("{value:.0}"),
        TwoBarUnit::Score => format!("{value:.0}"),
    }
}

fn format_value(value: f64, unit: TwoBarUnit) -> String {
    match unit {
        TwoBarUnit::Ksloc => format!("{value:.1} KSLOC"),
        TwoBarUnit::Score => format!("{value:.0}/100"),
        TwoBarUnit::Milliseconds => format!("{value:.2} ms"),
    }
}

fn repository_value(
    comparison: &JankuraiComparison,
    repo_id: &str,
    value: impl Fn(&super::super::jankurai_compare::JankuraiRepository) -> f64,
) -> Option<f64> {
    comparison
        .repositories
        .iter()
        .find(|repo| repo.id == repo_id)
        .map(value)
}

fn code_shape_value(comparison: &JankuraiComparison, repo_id: &str) -> Option<f64> {
    comparison
        .code_shape
        .iter()
        .find(|row| row.repo_id == repo_id)
        .map(|row| row.score)
}

fn ns_to_ms(ns: u128) -> f64 {
    ns as f64 / 1_000_000.0
}
