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
    let width = 760.0;
    let height = 168.0;
    let left = 132.0;
    let right = 92.0;
    let top = 48.0;
    let bar_h = 24.0;
    let row_gap = 20.0;
    let axis_y = 138.0;
    let plot_w = width - left - right;
    let sqlite = summary.sqlite_reference_ksloc();
    let redline = summary.redlinedb_ksloc();
    let max = (sqlite.max(redline) / 20.0).ceil() * 20.0;
    let x_for = |value: f64| left + value / max.max(1.0) * plot_w;
    let bar_w = |value: f64| (x_for(value) - left).max(1.0);
    let grid_values = [0.0, 40.0, 80.0, 120.0, 160.0]
        .into_iter()
        .filter(|value| *value <= max + f64::EPSILON)
        .collect::<Vec<_>>();
    let redline_y = top;
    let sqlite_y = top + bar_h + row_gap;
    let mut out = generated_xml_header(
        "crates/{redlinedb,sql,kernel,ffi}/src",
        REPORT_REGEN_COMMAND,
    );
    out.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="ksloc-title ksloc-desc">
<title id="ksloc-title">SQLite vs RedlineDB production KSLOC, Updated {}</title>
<desc id="ksloc-desc">Production Rust source lines in RedlineDB core crates compared with a fixed SQLite source-line reference. RedlineDB has {:.1} KSLOC and SQLite has {:.1} KSLOC.</desc>
<text x="{left}" y="22" font-family="sans-serif" font-size="17" font-weight="700" fill="#f97316">Production source footprint</text>
<text x="{left}" y="39" font-family="sans-serif" font-size="12" fill="#fbbf24">Core RedlineDB crates scanned without tests, blank lines, or comments; updated {}</text>
"##,
        updated_date, redline, sqlite, updated_date
    ));
    for value in grid_values {
        let x = x_for(value);
        out.push_str(&format!(
            r##"<line x1="{x:.2}" y1="44" x2="{x:.2}" y2="{axis_y}" stroke="#f59e0b" opacity="0.35"/>
<text x="{x:.2}" y="156" font-family="sans-serif" font-size="10" fill="#fbbf24" text-anchor="middle">{value:.0}</text>
"##
        ));
    }
    out.push_str(&format!(
        r##"<line x1="{left}" y1="{axis_y}" x2="{:.2}" y2="{axis_y}" stroke="#fbbf24"/>
<text x="{:.2}" y="156" font-family="sans-serif" font-size="10" fill="#fbbf24" text-anchor="end">KSLOC</text>
"##,
        left + plot_w,
        left + plot_w + 54.0
    ));
    out.push_str(&format!(
        r##"<text x="20" y="{:.2}" font-family="sans-serif" font-size="13" fill="#f97316">RedlineDB</text>
<rect x="{left}" y="{redline_y}" width="{:.2}" height="{bar_h}" rx="3" fill="#10b981"/>
<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="12" fill="#fbbf24">{:.1} KSLOC</text>
<text x="20" y="{:.2}" font-family="sans-serif" font-size="13" fill="#f97316">SQLite</text>
<rect x="{left}" y="{sqlite_y}" width="{:.2}" height="{bar_h}" rx="3" fill="#e11d48"/>
<text x="{:.2}" y="{:.2}" font-family="sans-serif" font-size="12" font-weight="700" fill="#ffffff" text-anchor="end">{:.1} KSLOC</text>
</svg>
"##,
        redline_y + 16.5,
        bar_w(redline),
        x_for(redline) + 8.0,
        redline_y + 16.5,
        redline,
        sqlite_y + 16.5,
        bar_w(sqlite),
        (x_for(sqlite) - 8.0).max(left + 78.0),
        sqlite_y + 16.5,
        sqlite
    ));
    out
}
