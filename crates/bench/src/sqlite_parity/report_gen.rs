use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use super::cli::validate_known_case_ids;
use super::{catalog, jankurai_compare, performance_histogram, source_lines};
pub(super) use model::{JankuraiScore, ManifestJson, RankedCase, SummaryJson};

mod io;
mod model;
mod paper;
mod ranking;
mod readme;
mod svg;

#[cfg(test)]
mod tests;

use io::{
    check_files, display_path, existing_manifest_git_sha, expected_case_id_text, git_sha,
    normalized_command, repo_root_from_readme, sha256_file, sha256_hex, source_lines_csv,
    write_files,
};
use paper::paper_loc_writes;
use ranking::{build_report, parse_raw_records, ranked_csv};
use readme::{
    jankurai_breakdown_block, metrics_block, parse_jankurai_score, readme_block,
    replace_jankurai_badge, replace_jankurai_breakdown_block, replace_metrics_block,
    replace_parity_badges, replace_readme_block,
};
use svg::{
    beyond_sqlite_feature_progress_svg, code_shape_svg, jankurai_score_svg, ksloc_svg, latency_svg,
    median_test_performance_svg,
};

const README_BEGIN: &str = "<!-- sqlite-parity-report:begin -->";
const README_END: &str = "<!-- sqlite-parity-report:end -->";
const README_METRICS_BEGIN: &str = "<!-- sqlite-parity-metrics:begin -->";
const README_METRICS_END: &str = "<!-- sqlite-parity-metrics:end -->";
const README_JANKURAI_BREAKDOWN_BEGIN: &str = "<!-- sqlite-jankurai-breakdown:begin -->";
const README_JANKURAI_BREAKDOWN_END: &str = "<!-- sqlite-jankurai-breakdown:end -->";
const JANKURAI_BADGE_BEGIN: &str = "<!-- jankurai-score-badge:begin -->";
const JANKURAI_BADGE_END: &str = "<!-- jankurai-score-badge:end -->";
const LATENCY_TABLE_ANCHOR: &str = "sqlite-parity-ranked-latency-table";
const MIN_MEDIAN_IMPROVEMENT_PCT: f64 = -25.0;
const MIN_WORST_IMPROVEMENT_PCT: f64 = -80.0;
const MIN_FASTER_CASES: usize = 25;
const LATENCY_REFERENCE_FLOOR_NS: u128 = 3_000_000;

#[derive(Debug)]
pub struct ReportOptions {
    pub input: PathBuf,
    pub case_list: Option<PathBuf>,
    pub expected_case_ids: BTreeSet<String>,
    pub out_dir: PathBuf,
    pub readme: PathBuf,
    pub plot: PathBuf,
    pub ksloc_plot: PathBuf,
    pub performance_histogram_plot: Option<PathBuf>,
    pub median_test_performance_plot: Option<PathBuf>,
    pub jankurai_score: Option<PathBuf>,
    pub jankurai_comparison: Option<PathBuf>,
    pub jankurai_comparison_plot: Option<PathBuf>,
    pub jankurai_score_plot: Option<PathBuf>,
    pub code_shape_plot: Option<PathBuf>,
    pub updated_date: String,
    pub expected_repetitions: Option<usize>,
    pub expected_warmup: Option<usize>,
    pub check: bool,
    pub command: Vec<String>,
}

pub fn generate(options: ReportOptions) -> Result<()> {
    let all_cases = catalog::all_cases()?;
    validate_known_case_ids(&options.expected_case_ids, &all_cases)?;
    let repo_root = repo_root_from_readme(&options.readme);
    let source_summary = source_lines::scan_core_crates(&repo_root)?;
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read sqlite parity raw input {}", options.input.display()))?;
    let raw_records = parse_raw_records(&raw_text)?;
    let beyond_sqlite_backlog_text =
        fs::read_to_string(repo_root.join("docs/beyond-sqlite-gaps.md"))
            .with_context(|| "read beyond-SQLite backlog docs".to_owned())?;

    let raw_out = options.out_dir.join("raw.jsonl");
    let ranked_out = options.out_dir.join("ranked.csv");
    let ksloc_out = options.out_dir.join("ksloc.csv");
    let summary_out = options.out_dir.join("summary.json");
    let manifest_out = options.out_dir.join("manifest.json");
    let manifest_git_sha = if options.check {
        existing_manifest_git_sha(&manifest_out)?.unwrap_or_else(git_sha)
    } else {
        git_sha()
    };
    let report = build_report(
        &all_cases,
        &options.expected_case_ids,
        raw_records,
        &options.updated_date,
        &manifest_git_sha,
        options.expected_repetitions,
        options.expected_warmup,
    )?;
    if options.check && !report.coverage_failures.is_empty() {
        bail!(
            "sqlite parity full-corpus report is incomplete: {}",
            report.coverage_failures.join("; ")
        );
    }
    if options.check && !report.performance_failures.is_empty() {
        bail!(
            "sqlite parity full-corpus performance gate failed: {}",
            report.performance_failures.join("; ")
        );
    }

    let ranked_csv = ranked_csv(&report.ranked);
    let source_csv = source_lines_csv(&source_summary);
    let summary_json = serde_json::to_string_pretty(&report.summary)? + "\n";
    let svg = latency_svg(&report.ranked, &report.summary);
    let ksloc_svg = ksloc_svg(&source_summary, &options.updated_date);
    let beyond_sqlite_feature_progress_plot =
        PathBuf::from("assets/beyond-sqlite-feature-progress.svg");
    let beyond_sqlite_feature_progress_svg_text =
        beyond_sqlite_feature_progress_svg(&beyond_sqlite_backlog_text, &options.updated_date)?;
    let median_test_performance_svg = median_test_performance_svg(&report.summary);
    let histogram =
        performance_histogram::build(report.ranked.iter().map(|case| case.improvement_pct));
    let performance_histogram_svg = performance_histogram::svg(&histogram, &options.updated_date);
    let jankurai_comparison = options
        .jankurai_comparison
        .as_deref()
        .map(jankurai_compare::read_comparison)
        .transpose()?;
    let jankurai_comparison_svg = jankurai_comparison.as_ref().map(jankurai_compare::svg);
    let jankurai_score_svg = jankurai_comparison
        .as_ref()
        .and_then(|comparison| jankurai_score_svg(comparison, &options.updated_date));
    let code_shape_svg = jankurai_comparison
        .as_ref()
        .and_then(|comparison| code_shape_svg(comparison, &options.updated_date));
    if options.jankurai_score_plot.is_some() && jankurai_score_svg.is_none() {
        bail!("jankurai score plot requested but comparison data is unavailable");
    }
    if options.code_shape_plot.is_some() && code_shape_svg.is_none() {
        bail!("code shape plot requested but comparison data is unavailable");
    }
    let paper_writes = paper_loc_writes(&repo_root, &source_summary)?;
    let readme_text = fs::read_to_string(&options.readme)
        .with_context(|| format!("read README {}", options.readme.display()))?;
    let mut readme = replace_readme_block(
        &readme_text,
        &readme_block(
            &report.ranked,
            &report.summary,
            &options.plot,
            options.performance_histogram_plot.as_deref(),
        ),
    )?;
    readme = replace_metrics_block(
        &readme,
        &metrics_block(
            &beyond_sqlite_feature_progress_plot,
            &options.ksloc_plot,
            options.jankurai_score_plot.as_deref(),
            options.code_shape_plot.as_deref(),
            options.median_test_performance_plot.as_deref(),
        ),
    )?;
    if let Some(plot) = options.jankurai_comparison_plot.as_deref() {
        readme = replace_jankurai_breakdown_block(&readme, &jankurai_breakdown_block(plot))?;
    }
    readme = replace_parity_badges(&readme, &report.summary);
    if let Some(score_path) = &options.jankurai_score {
        let score_text = fs::read_to_string(score_path)
            .with_context(|| format!("read jankurai score {}", score_path.display()))?;
        let score = parse_jankurai_score(&score_text)
            .with_context(|| format!("parse jankurai score {}", score_path.display()))?;
        readme = replace_jankurai_badge(&readme, &score)?;
    }

    let mut input_hashes = BTreeMap::new();
    input_hashes.insert(
        options.input.display().to_string(),
        sha256_hex(raw_text.as_bytes()),
    );
    input_hashes.insert(
        "expected_case_ids".to_owned(),
        sha256_hex(expected_case_id_text(&options.expected_case_ids).as_bytes()),
    );
    if let Some(case_list) = &options.case_list {
        input_hashes.insert(case_list.display().to_string(), sha256_file(case_list)?);
    }
    if let Some(score_path) = &options.jankurai_score {
        input_hashes.insert(score_path.display().to_string(), sha256_file(score_path)?);
    }
    if let Some(comparison_path) = &options.jankurai_comparison {
        input_hashes.insert(
            comparison_path.display().to_string(),
            sha256_file(comparison_path)?,
        );
    }
    input_hashes.insert(
        "crates/bench/sqlite_parity/generated_manifest.json".to_owned(),
        sha256_hex(include_bytes!(
            "../../sqlite_parity/generated_manifest.json"
        )),
    );

    let mut output_hashes = BTreeMap::new();
    output_hashes.insert("raw.jsonl".to_owned(), sha256_hex(raw_text.as_bytes()));
    output_hashes.insert("ranked.csv".to_owned(), sha256_hex(ranked_csv.as_bytes()));
    output_hashes.insert("ksloc.csv".to_owned(), sha256_hex(source_csv.as_bytes()));
    output_hashes.insert(
        "summary.json".to_owned(),
        sha256_hex(summary_json.as_bytes()),
    );
    output_hashes.insert(
        options.plot.display().to_string(),
        sha256_hex(svg.as_bytes()),
    );
    output_hashes.insert(
        beyond_sqlite_feature_progress_plot.display().to_string(),
        sha256_hex(beyond_sqlite_feature_progress_svg_text.as_bytes()),
    );
    output_hashes.insert(
        options.ksloc_plot.display().to_string(),
        sha256_hex(ksloc_svg.as_bytes()),
    );
    if let Some(path) = &options.median_test_performance_plot {
        output_hashes.insert(
            path.display().to_string(),
            sha256_hex(median_test_performance_svg.as_bytes()),
        );
    }
    if let Some(path) = &options.performance_histogram_plot {
        output_hashes.insert(
            path.display().to_string(),
            sha256_hex(performance_histogram_svg.as_bytes()),
        );
    }
    if let (Some(path), Some(svg)) = (
        &options.jankurai_comparison_plot,
        jankurai_comparison_svg.as_ref(),
    ) {
        output_hashes.insert(path.display().to_string(), sha256_hex(svg.as_bytes()));
    }
    if let (Some(path), Some(svg)) = (&options.jankurai_score_plot, jankurai_score_svg.as_ref()) {
        output_hashes.insert(path.display().to_string(), sha256_hex(svg.as_bytes()));
    }
    if let (Some(path), Some(svg)) = (&options.code_shape_plot, code_shape_svg.as_ref()) {
        output_hashes.insert(path.display().to_string(), sha256_hex(svg.as_bytes()));
    }
    output_hashes.insert(
        options.readme.display().to_string(),
        sha256_hex(readme.as_bytes()),
    );
    for (path, contents) in &paper_writes {
        output_hashes.insert(
            display_path(path).to_string(),
            sha256_hex(contents.as_bytes()),
        );
    }
    let manifest = ManifestJson {
        command: normalized_command(&options.command),
        git_sha: manifest_git_sha,
        sqlite_version: report.summary.sqlite_version.clone(),
        updated_date: options.updated_date,
        repetitions: report.repetitions,
        warmup: report.warmup,
        input_hashes,
        output_hashes,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";

    let mut writes = vec![
        (raw_out, raw_text),
        (ranked_out, ranked_csv),
        (ksloc_out, source_csv),
        (summary_out, summary_json),
        (manifest_out, manifest_json),
        (options.plot, svg),
        (
            beyond_sqlite_feature_progress_plot,
            beyond_sqlite_feature_progress_svg_text,
        ),
        (options.ksloc_plot, ksloc_svg),
        (options.readme, readme),
    ];
    if let Some(path) = options.median_test_performance_plot {
        writes.push((path, median_test_performance_svg));
    }
    if let Some(path) = options.performance_histogram_plot {
        writes.push((path, performance_histogram_svg));
    }
    if let (Some(path), Some(svg)) = (options.jankurai_comparison_plot, jankurai_comparison_svg) {
        writes.push((path, svg));
    }
    if let (Some(path), Some(svg)) = (options.jankurai_score_plot, jankurai_score_svg) {
        writes.push((path, svg));
    }
    if let (Some(path), Some(svg)) = (options.code_shape_plot, code_shape_svg) {
        writes.push((path, svg));
    }
    writes.extend(paper_writes);
    if options.check {
        check_files(&writes)
    } else {
        write_files(&writes)
    }
}
