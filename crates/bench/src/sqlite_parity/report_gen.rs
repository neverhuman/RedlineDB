use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::cli::validate_known_case_ids;
use super::{catalog, jankurai_compare, performance_histogram, source_lines};

const README_BEGIN: &str = "<!-- sqlite-parity-report:begin -->";
const README_END: &str = "<!-- sqlite-parity-report:end -->";
const README_METRICS_BEGIN: &str = "<!-- sqlite-parity-metrics:begin -->";
const README_METRICS_END: &str = "<!-- sqlite-parity-metrics:end -->";
const JANKURAI_BADGE_BEGIN: &str = "<!-- jankurai-score-badge:begin -->";
const JANKURAI_BADGE_END: &str = "<!-- jankurai-score-badge:end -->";
const LATENCY_TABLE_ANCHOR: &str = "sqlite-parity-ranked-latency-table";
const MIN_MEDIAN_IMPROVEMENT_PCT: f64 = -25.0;
const MIN_WORST_IMPROVEMENT_PCT: f64 = -75.0;
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
    pub jankurai_score: Option<PathBuf>,
    pub jankurai_comparison: Option<PathBuf>,
    pub jankurai_comparison_plot: Option<PathBuf>,
    pub updated_date: String,
    pub check: bool,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    case_id: String,
    name: String,
    #[serde(default)]
    case_file: String,
    priority: String,
    profile: String,
    category: String,
    #[serde(default)]
    sample_role: String,
    #[serde(default)]
    repetition_index: Option<usize>,
    #[serde(default)]
    sqlite_version: Option<String>,
    status: String,
    reference_elapsed_ns: u128,
    target_elapsed_ns: u128,
}

#[derive(Debug, Clone)]
struct RankedCase {
    case_id: String,
    name: String,
    case_file: String,
    priority: String,
    profile: String,
    category: String,
    sqlite_median_ns: u128,
    redline_median_ns: u128,
    improvement_pct: f64,
    samples: usize,
}

#[derive(Debug, Serialize)]
struct SummaryJson {
    updated_date: String,
    git_sha: String,
    sqlite_version: String,
    generated_cases: usize,
    expected_cases: usize,
    passed_cases: usize,
    failed_cases: usize,
    missing_cases: usize,
    skipped_cases: usize,
    ranked_cases: usize,
    coverage_pct: f64,
    measured_samples: usize,
    warmup_samples: usize,
    median_latency_gap_pct: f64,
    worst_latency_gap_pct: f64,
    faster_cases: usize,
    latency_reference_floor_ns: u128,
}

#[derive(Debug, Serialize)]
struct ManifestJson {
    command: Vec<String>,
    git_sha: String,
    sqlite_version: String,
    updated_date: String,
    repetitions: usize,
    warmup: usize,
    input_hashes: BTreeMap<String, String>,
    output_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JankuraiScore {
    score: u64,
    status: String,
    color: &'static str,
}

pub fn generate(options: ReportOptions) -> Result<()> {
    let all_cases = catalog::all_cases()?;
    validate_known_case_ids(&options.expected_case_ids, &all_cases)?;
    let repo_root = repo_root_from_readme(&options.readme);
    let source_summary = source_lines::scan_core_crates(&repo_root)?;
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read sqlite parity raw input {}", options.input.display()))?;
    let raw_records = parse_raw_records(&raw_text)?;

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
    let histogram =
        performance_histogram::build(report.ranked.iter().map(|case| case.improvement_pct));
    let performance_histogram_svg = performance_histogram::svg(&histogram, &options.updated_date);
    let jankurai_comparison = options
        .jankurai_comparison
        .as_deref()
        .map(jankurai_compare::read_comparison)
        .transpose()?;
    let jankurai_comparison_svg = jankurai_comparison.as_ref().map(jankurai_compare::svg);
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
            &options.ksloc_plot,
            options.jankurai_comparison_plot.as_deref(),
        ),
    )?;
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
        options.ksloc_plot.display().to_string(),
        sha256_hex(ksloc_svg.as_bytes()),
    );
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
        (options.ksloc_plot, ksloc_svg),
        (options.readme, readme),
    ];
    if let Some(path) = options.performance_histogram_plot {
        writes.push((path, performance_histogram_svg));
    }
    if let (Some(path), Some(svg)) = (options.jankurai_comparison_plot, jankurai_comparison_svg) {
        writes.push((path, svg));
    }
    writes.extend(paper_writes);
    if options.check {
        check_files(&writes)
    } else {
        write_files(&writes)
    }
}

#[derive(Debug)]
struct BuiltReport {
    ranked: Vec<RankedCase>,
    summary: SummaryJson,
    repetitions: usize,
    warmup: usize,
    coverage_failures: Vec<String>,
    performance_failures: Vec<String>,
}

fn parse_raw_records(raw_text: &str) -> Result<Vec<RawRecord>> {
    let mut records = Vec::new();
    for (index, line) in raw_text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        records.push(serde_json::from_str(line).with_context(|| {
            format!(
                "parse sqlite parity raw JSONL line {}",
                index.saturating_add(1)
            )
        })?);
    }
    Ok(records)
}

fn build_report(
    all_cases: &[super::case::Case],
    expected: &BTreeSet<String>,
    raw_records: Vec<RawRecord>,
    updated_date: &str,
    git_sha: &str,
) -> Result<BuiltReport> {
    let mut grouped = BTreeMap::<String, Vec<RawRecord>>::new();
    let mut sqlite_version = String::from("<unknown>");
    let mut warmup = 0usize;
    for record in raw_records {
        if sqlite_version == "<unknown>"
            && let Some(version) = &record.sqlite_version
            && !version.is_empty()
        {
            sqlite_version = version.clone();
        }
        if is_warmup(&record) {
            warmup = warmup.saturating_add(1);
        }
        grouped
            .entry(record.case_id.clone())
            .or_default()
            .push(record);
    }

    let case_files = all_cases
        .iter()
        .map(|case| (case.display_id(), case.case_file_name()))
        .collect::<BTreeMap<_, _>>();
    let mut ranked = Vec::new();
    let mut passed_cases = 0usize;
    let mut failed_cases = 0usize;
    let mut missing_cases = 0usize;
    let mut skipped_cases = 0usize;
    let mut measured_samples = 0usize;
    let mut coverage_failures = Vec::new();
    for id in expected {
        let Some(records) = grouped.get(id) else {
            missing_cases = missing_cases.saturating_add(1);
            coverage_failures.push(format!("{id} missing"));
            continue;
        };
        if records.iter().any(|record| record.status == "skipped") {
            skipped_cases = skipped_cases.saturating_add(1);
            coverage_failures.push(format!("{id} skipped"));
            continue;
        }
        if records.iter().any(|record| record.status == "failed") {
            failed_cases = failed_cases.saturating_add(1);
            coverage_failures.push(format!("{id} failed"));
            continue;
        }
        let passed = records
            .iter()
            .filter(|record| record.status == "passed" && is_measured(record))
            .collect::<Vec<_>>();
        if passed.is_empty() {
            missing_cases = missing_cases.saturating_add(1);
            coverage_failures.push(format!("{id} lacks measured samples"));
            continue;
        }
        passed_cases = passed_cases.saturating_add(1);
        measured_samples = measured_samples.saturating_add(passed.len());
        let sqlite_median_ns = median_u128(
            passed
                .iter()
                .map(|record| record.reference_elapsed_ns)
                .collect(),
        );
        let redline_median_ns = median_u128(
            passed
                .iter()
                .map(|record| record.target_elapsed_ns)
                .collect(),
        );
        let first = passed[0];
        let case_file = if first.case_file.is_empty() {
            case_files.get(id).cloned().with_context(|| {
                format!("resolve sqlite parity case file metadata for expected case {id}")
            })?
        } else {
            first.case_file.clone()
        };
        ranked.push(RankedCase {
            case_id: id.clone(),
            name: first.name.clone(),
            case_file,
            priority: first.priority.clone(),
            profile: first.profile.clone(),
            category: first.category.clone(),
            sqlite_median_ns,
            redline_median_ns,
            improvement_pct: improvement_pct(sqlite_median_ns, redline_median_ns),
            samples: passed.len(),
        });
    }
    ranked.sort_by(|left, right| {
        left.improvement_pct
            .total_cmp(&right.improvement_pct)
            .then_with(|| left.case_id.cmp(&right.case_id))
    });

    let generated_cases = all_cases.len();
    let expected_cases = expected.len();
    let coverage_pct = passed_cases as f64 / expected_cases.max(1) as f64 * 100.0;
    let repetitions = ranked.iter().map(|case| case.samples).max().unwrap_or(0);
    let warmup_per_case = if passed_cases == 0 {
        0
    } else {
        warmup / passed_cases
    };
    let ranked_cases = ranked.len();
    let median_latency_gap_pct = median_improvement_pct(&ranked);
    let worst_latency_gap_pct = ranked
        .first()
        .map(|case| case.improvement_pct)
        .unwrap_or(0.0);
    let faster_cases = ranked
        .iter()
        .filter(|case| case.improvement_pct > 0.0)
        .count();
    let performance_failures =
        performance_failures(median_latency_gap_pct, worst_latency_gap_pct, faster_cases);
    Ok(BuiltReport {
        ranked,
        summary: SummaryJson {
            updated_date: updated_date.to_owned(),
            git_sha: git_sha.to_owned(),
            sqlite_version,
            generated_cases,
            expected_cases,
            passed_cases,
            failed_cases,
            missing_cases,
            skipped_cases,
            ranked_cases,
            coverage_pct,
            measured_samples,
            warmup_samples: warmup,
            median_latency_gap_pct,
            worst_latency_gap_pct,
            faster_cases,
            latency_reference_floor_ns: LATENCY_REFERENCE_FLOOR_NS,
        },
        repetitions,
        warmup: warmup_per_case,
        coverage_failures,
        performance_failures,
    })
}

fn is_warmup(record: &RawRecord) -> bool {
    record.sample_role == "warmup"
}

fn is_measured(record: &RawRecord) -> bool {
    record.status == "passed"
        && (record.repetition_index.is_some()
            || record.sample_role.starts_with("measured")
            || record.sample_role.is_empty())
}

fn median_u128(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn improvement_pct(sqlite_median_ns: u128, redline_median_ns: u128) -> f64 {
    let effective_sqlite_ns = sqlite_median_ns.max(LATENCY_REFERENCE_FLOOR_NS);
    (effective_sqlite_ns as f64 - redline_median_ns as f64) / effective_sqlite_ns.max(1) as f64
        * 100.0
}

fn median_improvement_pct(ranked: &[RankedCase]) -> f64 {
    if ranked.is_empty() {
        0.0
    } else {
        ranked[ranked.len() / 2].improvement_pct
    }
}

fn performance_failures(
    median_latency_gap_pct: f64,
    worst_latency_gap_pct: f64,
    faster_cases: usize,
) -> Vec<String> {
    let mut failures = Vec::new();
    if median_latency_gap_pct < MIN_MEDIAN_IMPROVEMENT_PCT {
        failures.push(format!(
            "median latency gap {:.2}% < {:.2}%",
            median_latency_gap_pct, MIN_MEDIAN_IMPROVEMENT_PCT
        ));
    }
    if worst_latency_gap_pct <= MIN_WORST_IMPROVEMENT_PCT {
        failures.push(format!(
            "worst latency gap {:.2}% <= {:.2}%",
            worst_latency_gap_pct, MIN_WORST_IMPROVEMENT_PCT
        ));
    }
    if faster_cases < MIN_FASTER_CASES {
        failures.push(format!("faster cases {faster_cases} < {MIN_FASTER_CASES}"));
    }
    failures
}

fn ranked_csv(ranked: &[RankedCase]) -> String {
    let mut out = String::from(
        "rank,case_id,name,case_file,priority,profile,category,sqlite_median_ns,redline_median_ns,improvement_pct,samples\n",
    );
    for (index, row) in ranked.iter().enumerate() {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{:.6},{}\n",
            index.saturating_add(1),
            row.case_id,
            csv(&row.name),
            csv(&row.case_file),
            row.priority,
            row.profile,
            csv(&row.category),
            row.sqlite_median_ns,
            row.redline_median_ns,
            row.improvement_pct,
            row.samples
        ));
    }
    out
}

fn source_lines_csv(summary: &source_lines::SourceLineSummary) -> String {
    let mut out = String::from("component,loc,ksloc,files,source_path,notes\n");
    for component in &summary.components {
        out.push_str(&format!(
            "{},{},{:.1},{},{},{}\n",
            component.id,
            component.lines,
            source_lines::ksloc(component.lines),
            component.files,
            csv(&component.path),
            csv(&component.notes)
        ));
    }
    out.push_str(&format!(
        "redlinedb_core_total,{},{:.1},{},{},{}\n",
        summary.total_lines,
        summary.redlinedb_ksloc(),
        summary.total_files,
        csv("crates/{redlinedb,sql,kernel,ffi}/src"),
        csv("production Rust source; excludes tests, benches, examples, cfg(test) items, blank lines, and comments")
    ));
    out.push_str(&format!(
        "sqlite_reference,{},{:.1},,fixed,{}\n",
        summary.sqlite_reference_lines,
        summary.sqlite_reference_ksloc(),
        csv("fixed SQLite source-line reference")
    ));
    out
}

fn readme_block(
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
            "| {} | [{} {}](crates/bench/sqlite_parity/cases/{}) | {} | {} | {} | {} | {} | {} |\n",
            index.saturating_add(1),
            row.case_id,
            escape_md(&row.name),
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

fn metrics_block(ksloc_plot: &Path, jankurai_comparison_plot: Option<&Path>) -> String {
    let mut out = format!(
        "## Engine Metrics\n\n{README_METRICS_BEGIN}\n\n![SQLite vs RedlineDB production KSLOC chart]({})\n\n",
        ksloc_plot.display()
    );
    if let Some(plot) = jankurai_comparison_plot {
        out.push_str(&format!(
            "![RedlineDB vs SQLite Jankurai comparison chart]({})\n\n",
            plot.display()
        ));
    }
    out.push_str(README_METRICS_END);
    out.push('\n');
    out
}

fn replace_metrics_block(readme: &str, block: &str) -> Result<String> {
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

fn replace_readme_block(readme: &str, block: &str) -> Result<String> {
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

fn parse_jankurai_score(score_json: &str) -> Result<JankuraiScore> {
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

fn replace_jankurai_badge(readme: &str, score: &JankuraiScore) -> Result<String> {
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

fn replace_parity_badges(readme: &str, summary: &SummaryJson) -> String {
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

fn jankurai_badge_block(score: &JankuraiScore) -> String {
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

fn latency_svg(ranked: &[RankedCase], summary: &SummaryJson) -> String {
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
    let mut out = String::new();
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
                xml(&row.name),
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

fn ksloc_svg(summary: &source_lines::SourceLineSummary, updated_date: &str) -> String {
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
    let mut out = String::new();
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

fn paper_loc_writes(
    repo_root: &Path,
    summary: &source_lines::SourceLineSummary,
) -> Result<Vec<(PathBuf, String)>> {
    let data_path = repo_root.join("paper/data/loc_comparison.csv");
    let implementation_path = repo_root.join("paper/sections/implementation.tex");
    let abstract_path = repo_root.join("paper/sections/abstract.tex");
    let introduction_path = repo_root.join("paper/sections/introduction.tex");
    let evaluation_path = repo_root.join("paper/sections/evaluation.tex");
    let conclusion_path = repo_root.join("paper/sections/conclusion.tex");

    let implementation = replace_loc_block(
        &read_text(&implementation_path)?,
        "implementation",
        r"\subsection{Lines of Code}",
        r"\subsection{Failpoint Discipline}",
        &implementation_loc_block(summary),
    )?;
    let abstract_text = replace_loc_block(
        &read_text(&abstract_path)?,
        "abstract",
        "RedlineDB exposes the\n",
        "single-writer WAL with",
        &abstract_loc_block(summary),
    )?;
    let introduction = replace_loc_block(
        &read_text(&introduction_path)?,
        "introduction",
        "The kernel, parser, planner, executor, public Rust facade, and\n",
        "The kernel achieves",
        &introduction_loc_block(summary),
    )?;
    let evaluation = replace_loc_metric_row(&read_text(&evaluation_path)?, summary)?;
    let conclusion = replace_loc_block(
        &read_text(&conclusion_path)?,
        "conclusion",
        "buys. The kernel and SQL layer together fit",
        "On a 128-vCPU host",
        &conclusion_loc_block(summary),
    )?;

    Ok(vec![
        (data_path, source_lines_csv(summary)),
        (implementation_path, implementation),
        (abstract_path, abstract_text),
        (introduction_path, introduction),
        (evaluation_path, evaluation),
        (conclusion_path, conclusion),
    ])
}

fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
}

fn replace_loc_block(
    text: &str,
    id: &str,
    legacy_start: &str,
    legacy_end: &str,
    replacement: &str,
) -> Result<String> {
    let begin = format!("% sqlite-parity-loc:{id}:begin");
    let end = format!("% sqlite-parity-loc:{id}:end");
    if let (Some(start), Some(end_start)) = (text.find(&begin), text.find(&end)) {
        let end_index = consume_line_endings(text, end_start + end.len());
        let mut next = String::new();
        next.push_str(&text[..start]);
        next.push_str(replacement);
        next.push_str(&text[end_index..]);
        return Ok(next);
    }
    let Some(start) = text.find(legacy_start) else {
        bail!("paper LOC block `{id}` lacks start marker");
    };
    let Some(relative_end) = text[start..].find(legacy_end) else {
        bail!("paper LOC block `{id}` lacks end marker");
    };
    let end_index = start + relative_end;
    let mut next = String::new();
    next.push_str(&text[..start]);
    next.push_str(replacement);
    next.push_str(&text[end_index..]);
    Ok(next)
}

fn consume_line_endings(text: &str, index: usize) -> usize {
    let mut cursor = index;
    while cursor < text.len() {
        if text[cursor..].starts_with("\r\n") {
            cursor += 2;
        } else if text[cursor..].starts_with('\n') || text[cursor..].starts_with('\r') {
            cursor += 1;
        } else {
            break;
        }
    }
    cursor
}

fn implementation_loc_block(summary: &source_lines::SourceLineSummary) -> String {
    let mut out = String::new();
    out.push_str("% sqlite-parity-loc:implementation:begin\n");
    out.push_str("\\subsection{Lines of Code}\n");
    out.push_str("Table~\\ref{tab:loc} reports the current production source\n");
    out.push_str("breakdown for the core RedlineDB engine crates. The counts are\n");
    out.push_str("generated by the SQLite parity report scanner over\n");
    out.push_str("\\texttt{crates/redlinedb/src}, \\texttt{crates/sql/src},\n");
    out.push_str("\\texttt{crates/kernel/src}, and \\texttt{crates/ffi/src}; the\n");
    out.push_str("scanner excludes test, bench, and example folders, files named\n");
    out.push_str("\\texttt{tests.rs}, \\texttt{\\#[cfg(test)]} items, blank lines,\n");
    out.push_str("and Rust comments. The current\n");
    out.push_str(&format!(
        "core total is {} lines ({:.1} KSLOC). SQLite is shown as the\n",
        tex_int(summary.total_lines),
        summary.redlinedb_ksloc()
    ));
    out.push_str("fixed 155.8 KSLOC source-line reference.\n\n");
    out.push_str("\\begin{table}[t]\n\\centering\n");
    out.push_str("\\caption{Scanner-counted production Rust source lines vs SQLite reference}\n");
    out.push_str("\\label{tab:loc}\n\\renewcommand{\\arraystretch}{1.05}\n");
    out.push_str("\\begin{tabular}{@{}lrr@{}}\n\\toprule\n");
    out.push_str("\\textbf{Component} & \\textbf{LOC} & \\textbf{KSLOC} \\\\\n\\midrule\n");
    for component in &summary.components {
        out.push_str(&format!(
            "\\texttt{{{}}} & {} & {:.1} \\\\\n",
            component.label,
            tex_int(component.lines),
            source_lines::ksloc(component.lines)
        ));
    }
    out.push_str("\\midrule\n");
    out.push_str(&format!(
        "\\textbf{{RedlineDB core production source}} & \\textbf{{{}}} & \\textbf{{{:.1}}} \\\\\n",
        tex_int(summary.total_lines),
        summary.redlinedb_ksloc()
    ));
    out.push_str("\\midrule\n");
    out.push_str(&format!(
        "SQLite source-line reference & {} & {:.1} \\\\\n",
        tex_int(summary.sqlite_reference_lines),
        summary.sqlite_reference_ksloc()
    ));
    out.push_str("\\bottomrule\n\\end{tabular}\n\\end{table}\n\n");
    out.push_str("% sqlite-parity-loc:implementation:end\n\n");
    out
}

fn abstract_loc_block(summary: &source_lines::SourceLineSummary) -> String {
    format!(
        "% sqlite-parity-loc:abstract:begin\nRedlineDB exposes the\n\\texttt{{sqlite3.h}} C ABI and a native \\texttt{{rldb\\_*}} surface from\n{:.1} KSLOC of scanner-counted core Rust source versus SQLite's fixed\n155.8 KSLOC reference, and replaces the\n% sqlite-parity-loc:abstract:end\n",
        summary.redlinedb_ksloc()
    )
}

fn introduction_loc_block(summary: &source_lines::SourceLineSummary) -> String {
    format!(
        "% sqlite-parity-loc:introduction:begin\nThe kernel, parser, planner, executor, public Rust facade, and\nSQLite C ABI shim together total {:.1} KSLOC of scanner-counted\nproduction Rust source across four core crates (Table~\\ref{{tab:loc}});\nonly the FFI shim contains \\texttt{{unsafe}} blocks, and those exist\nbecause the C ABI requires them.\n% sqlite-parity-loc:introduction:end\n",
        summary.redlinedb_ksloc()
    )
}

fn conclusion_loc_block(summary: &source_lines::SourceLineSummary) -> String {
    format!(
        "% sqlite-parity-loc:conclusion:begin\nbuys. The core RedlineDB engine crates total {:.1} KSLOC of scanner-counted\nproduction Rust source, compared with SQLite's fixed 155.8 KSLOC\nsource-line reference.\n% sqlite-parity-loc:conclusion:end\n",
        summary.redlinedb_ksloc()
    )
}

fn replace_loc_metric_row(text: &str, summary: &source_lines::SourceLineSummary) -> Result<String> {
    let new_line = format!(
        "Core production Rust source LOC & {} \\\\",
        tex_int(summary.total_lines)
    );
    let mut replaced = false;
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("Phase-10 active source LOC &")
            || line.starts_with("Core production Rust source LOC &")
        {
            out.push_str(&new_line);
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !replaced {
        bail!("paper evaluation LOC metric row not found");
    }
    Ok(out)
}

fn repo_root_from_readme(readme: &Path) -> PathBuf {
    readme
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn tex_int(value: usize) -> String {
    comma_int(value).replace(',', "{,}")
}

fn comma_int(value: usize) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn check_files(writes: &[(PathBuf, String)]) -> Result<()> {
    let mut drifted = Vec::new();
    for (path, contents) in writes {
        let current = fs::read_to_string(path).unwrap_or_default();
        if current != *contents {
            drifted.push(path.display().to_string());
        }
    }
    if !drifted.is_empty() {
        bail!("sqlite parity report drift: {}", drifted.join(", "));
    }
    Ok(())
}

fn write_files(writes: &[(PathBuf, String)]) -> Result<()> {
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

fn color(value: f64) -> &'static str {
    if value < -5.0 {
        "#dc2626"
    } else if value < -0.5 {
        "#f97316"
    } else if value <= 0.5 {
        "#6b7280"
    } else if value <= 5.0 {
        "#16a34a"
    } else {
        "#2563eb"
    }
}

fn improvement_cell(value: f64) -> String {
    format!(
        r#"<span style="color:{}">{:.2}%</span>"#,
        color(value),
        value
    )
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn escape_md(value: &str) -> String {
    value.replace('|', "\\|")
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn display_path(path: &Path) -> std::borrow::Cow<'_, str> {
    path.to_string_lossy()
}

fn expected_case_id_text(ids: &BTreeSet<String>) -> String {
    let mut out = ids.iter().cloned().collect::<Vec<_>>().join("\n");
    out.push('\n');
    out
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("hash {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git_sha() -> String {
    resolve_git_sha(env_git_sha(), git_head_sha)
}

fn resolve_git_sha(env_sha: Option<String>, git_fallback: impl FnOnce() -> String) -> String {
    env_sha.unwrap_or_else(git_fallback)
}

fn git_head_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn env_git_sha() -> Option<String> {
    normalize_git_sha(std::env::var("REDLINEDB_BENCH_GIT_SHA").ok())
}

fn normalize_git_sha(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_owned())
        .filter(|candidate| !candidate.is_empty())
}

fn existing_manifest_git_sha(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let manifest = fs::read_to_string(path)
        .with_context(|| format!("read existing manifest {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&manifest)
        .with_context(|| format!("parse existing manifest {}", path.display()))?;
    Ok(value
        .get("git_sha")
        .and_then(|git_sha| git_sha.as_str())
        .map(str::to_owned)
        .and_then(|git_sha| normalize_git_sha(Some(git_sha))))
}

fn normalized_command(command: &[String]) -> Vec<String> {
    command
        .iter()
        .filter(|arg| arg.as_str() != "--check")
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(case_id: &str, sqlite: u128, redline: u128) -> RawRecord {
        RawRecord {
            case_id: case_id.to_owned(),
            name: format!("CASE_{case_id}"),
            case_file: format!("SQLITE_PARITY_{case_id}.rs"),
            priority: "P0".to_owned(),
            profile: "memory".to_owned(),
            category: "fixture".to_owned(),
            sample_role: "measured:1".to_owned(),
            repetition_index: Some(1),
            sqlite_version: Some("3.fixture".to_owned()),
            status: "passed".to_owned(),
            reference_elapsed_ns: sqlite,
            target_elapsed_ns: redline,
        }
    }

    fn fixture_ranked() -> Vec<RankedCase> {
        vec![RankedCase {
            case_id: "00001".to_owned(),
            name: "fixture".to_owned(),
            case_file: "fixture.rs".to_owned(),
            priority: "P0".to_owned(),
            profile: "memory".to_owned(),
            category: "fixture".to_owned(),
            sqlite_median_ns: 100,
            redline_median_ns: 90,
            improvement_pct: 10.0,
            samples: 1,
        }]
    }

    fn fixture_summary() -> SummaryJson {
        SummaryJson {
            updated_date: "2026-05-20".to_owned(),
            git_sha: "fixture-sha".to_owned(),
            sqlite_version: "3.fixture".to_owned(),
            generated_cases: 1127,
            expected_cases: 612,
            passed_cases: 612,
            failed_cases: 0,
            missing_cases: 0,
            skipped_cases: 0,
            ranked_cases: 1,
            coverage_pct: 100.0,
            measured_samples: 1,
            warmup_samples: 0,
            median_latency_gap_pct: 10.0,
            worst_latency_gap_pct: 10.0,
            faster_cases: 1,
            latency_reference_floor_ns: LATENCY_REFERENCE_FLOOR_NS,
        }
    }

    #[test]
    fn improvement_sign_convention_and_ranking() {
        let mut ranked = vec![
            RankedCase {
                improvement_pct: improvement_pct(4_000_000, 8_000_000),
                case_id: "00001".to_owned(),
                name: "regression".to_owned(),
                case_file: "a.rs".to_owned(),
                priority: "P0".to_owned(),
                profile: "memory".to_owned(),
                category: "fixture".to_owned(),
                sqlite_median_ns: 4_000_000,
                redline_median_ns: 8_000_000,
                samples: 1,
            },
            RankedCase {
                improvement_pct: improvement_pct(4_000_000, 2_000_000),
                case_id: "00002".to_owned(),
                name: "gain".to_owned(),
                case_file: "b.rs".to_owned(),
                priority: "P0".to_owned(),
                profile: "memory".to_owned(),
                category: "fixture".to_owned(),
                sqlite_median_ns: 4_000_000,
                redline_median_ns: 2_000_000,
                samples: 1,
            },
        ];
        ranked.sort_by(|left, right| {
            left.improvement_pct
                .total_cmp(&right.improvement_pct)
                .then_with(|| left.case_id.cmp(&right.case_id))
        });
        assert_eq!(ranked[0].case_id, "00001");
        assert_eq!(ranked[1].case_id, "00002");
        assert_eq!(ranked[0].improvement_pct, -100.0);
        assert_eq!(ranked[1].improvement_pct, 50.0);
    }

    #[test]
    fn medians_exclude_warmup() {
        let mut records = vec![raw("00001", 100, 90), raw("00001", 300, 120)];
        records.push(RawRecord {
            sample_role: "warmup".to_owned(),
            repetition_index: None,
            reference_elapsed_ns: 9_999,
            target_elapsed_ns: 9_999,
            ..raw("00001", 9_999, 9_999)
        });
        let all_cases = catalog::all_cases().expect("manifest");
        let expected = BTreeSet::from(["00001".to_owned()]);
        let report =
            build_report(&all_cases, &expected, records, "2026-05-20", "sha").expect("report");
        assert_eq!(report.ranked[0].sqlite_median_ns, 300);
        assert_eq!(report.ranked[0].redline_median_ns, 120);
        assert_eq!(report.summary.warmup_samples, 1);
    }

    #[test]
    fn performance_histogram_uses_measured_case_medians() {
        let mut records = vec![raw("00001", 4_000_000, 2_000_000)];
        records.push(RawRecord {
            sample_role: "warmup".to_owned(),
            repetition_index: None,
            reference_elapsed_ns: 4_000_000,
            target_elapsed_ns: 8_000_000,
            ..raw("00001", 4_000_000, 8_000_000)
        });
        let all_cases = catalog::all_cases().expect("manifest");
        let expected = BTreeSet::from(["00001".to_owned()]);
        let report =
            build_report(&all_cases, &expected, records, "2026-05-20", "sha").expect("report");
        let histogram =
            performance_histogram::build(report.ranked.iter().map(|case| case.improvement_pct));

        assert_eq!(histogram.case_count, 1);
        assert_eq!(histogram.min_pct, 50.0);
        assert_eq!(histogram.median_pct, 50.0);
        assert_eq!(histogram.max_pct, 50.0);
    }

    #[test]
    fn report_counts_missing_failed_and_skipped_cases() {
        let all_cases = catalog::all_cases().expect("manifest");
        let expected = BTreeSet::from([
            "00001".to_owned(),
            "00002".to_owned(),
            "00003".to_owned(),
            "00004".to_owned(),
        ]);
        let mut failed = raw("00002", 100, 90);
        failed.status = "failed".to_owned();
        let mut skipped = raw("00003", 100, 90);
        skipped.status = "skipped".to_owned();
        let report = build_report(
            &all_cases,
            &expected,
            vec![raw("00001", 100, 90), failed, skipped],
            "2026-05-20",
            "sha",
        )
        .expect("report");

        assert_eq!(report.summary.passed_cases, 1);
        assert_eq!(report.summary.failed_cases, 1);
        assert_eq!(report.summary.skipped_cases, 1);
        assert_eq!(report.summary.missing_cases, 1);
        assert_eq!(report.summary.coverage_pct, 25.0);
        assert_eq!(report.coverage_failures.len(), 3);
    }

    #[test]
    fn missing_case_file_metadata_fails_closed() {
        let records = vec![RawRecord {
            case_file: String::new(),
            ..raw("99999", 100, 90)
        }];
        let all_cases = Vec::new();
        let expected = BTreeSet::from(["99999".to_owned()]);

        let err = build_report(&all_cases, &expected, records, "2026-05-20", "sha")
            .expect_err("metadata");

        assert!(
            err.to_string()
                .contains("resolve sqlite parity case file metadata for expected case 99999")
        );
    }

    #[test]
    fn readme_marker_replacement_preserves_surrounding_content() {
        let current = format!("before\n{README_BEGIN}\nold\n{README_END}\nafter\n");
        let next = replace_readme_block(&current, "new block\n").expect("replace");
        assert_eq!(next, "before\nnew block\nafter\n");
    }

    #[test]
    fn readme_replacement_removes_outer_details_wrapper() {
        let current = format!(
            "before\n<details>\n<summary>Detailed parity report</summary>\n\n{README_BEGIN}\nold\n{README_END}\n\n</details>\nafter\n"
        );
        let next = replace_readme_block(&current, "new block\n").expect("replace");
        assert_eq!(next, "before\nnew block\nafter\n");
    }

    #[test]
    fn readme_block_includes_visible_charts_and_latency_anchor() {
        let block = readme_block(
            &fixture_ranked(),
            &fixture_summary(),
            Path::new("assets/sqlite-parity-latency-gap.svg"),
            Some(Path::new("assets/sqlite-parity-performance-histogram.svg")),
        );

        assert!(block.contains(
            "![SQLite parity latency improvement plot](assets/sqlite-parity-latency-gap.svg)"
        ));
        assert!(block.contains(
            "![SQLite parity performance distribution](assets/sqlite-parity-performance-histogram.svg)"
        ));
        assert!(!block.contains("sqlite-parity-ksloc.svg"));
        let metrics = metrics_block(
            Path::new("assets/sqlite-parity-ksloc.svg"),
            Some(Path::new("assets/sqlite-jankurai-comparison.svg")),
        );
        assert!(block.contains("[Full ranked latency table](#sqlite-parity-ranked-latency-table)"));
        assert!(metrics.contains(
            "![SQLite vs RedlineDB production KSLOC chart](assets/sqlite-parity-ksloc.svg)"
        ));
        assert!(metrics.contains(
            "![RedlineDB vs SQLite Jankurai comparison chart](assets/sqlite-jankurai-comparison.svg)"
        ));
        assert!(block.contains("<details id=\"sqlite-parity-ranked-latency-table\">"));
    }

    #[test]
    fn jankurai_badge_renders_score_status_and_color() {
        let score =
            parse_jankurai_score(r#"{ "score": 64, "decision": { "status": "advisory" } }"#)
                .expect("score");
        let badge = jankurai_badge_block(&score);

        assert_eq!(
            score,
            JankuraiScore {
                score: 64,
                status: "advisory".to_owned(),
                color: "orange",
            }
        );
        assert!(badge.contains("https://img.shields.io/badge/jankurai-64%2F100%20advisory-orange"));
        assert!(badge.contains("alt=\"jankurai score: 64/100 advisory\""));
    }

    #[test]
    fn jankurai_badge_replacement_preserves_static_badges() {
        let score = JankuraiScore {
            score: 64,
            status: "advisory".to_owned(),
            color: "orange",
        };
        let current = "<p align=\"center\">\n  <img src=\"assets/redlinedb-banner.png\" alt=\"RedlineDB\" width=\"100%\">\n</p>\n\n<p align=\"center\">\n  <a href=\"LICENSE\"><img src=\"license.svg\" alt=\"license\"></a>\n  <img src=\"https://img.shields.io/badge/version-1.0.26-blue\" alt=\"version\">\n</p>\nafter\n";
        let next = replace_jankurai_badge(current, &score).expect("replace");

        assert!(next.contains("<a href=\"LICENSE\"><img src=\"license.svg\" alt=\"license\"></a>"));
        assert!(next.contains(
            "<img src=\"https://img.shields.io/badge/version-1.0.26-blue\" alt=\"version\">"
        ));
        assert!(next.contains(JANKURAI_BADGE_BEGIN));
        assert!(next.contains(JANKURAI_BADGE_END));
        assert!(
            next.find("assets/redlinedb-banner.png")
                .expect("banner paragraph")
                < next.find(JANKURAI_BADGE_BEGIN).expect("badge marker")
        );
    }

    #[test]
    fn svg_contains_required_labels() {
        let ranked = fixture_ranked();
        let summary = fixture_summary();
        let svg = latency_svg(&ranked, &summary);
        assert!(svg.contains("Updated 2026-05-20"));
        assert!(svg.contains("Floor-adjusted latency improvement vs SQLite (%)"));
        assert!(svg.contains("colormap legend"));
        assert!(svg.contains("0% horizontal reference line"));
    }

    #[test]
    fn ksloc_svg_uses_dark_background_safe_text_colors() {
        let summary = source_lines::SourceLineSummary {
            components: Vec::new(),
            total_files: 4,
            total_lines: 51_400,
            sqlite_reference_lines: 155_800,
        };
        let svg = ksloc_svg(&summary, "2026-05-20");

        assert!(svg.contains("fill=\"#f97316\""));
        assert!(svg.contains("fill=\"#fbbf24\""));
        assert!(!svg.contains("fill=\"#111827\""));
        assert!(!svg.contains("fill=\"#6b7280\""));
    }

    #[test]
    fn manifest_git_sha_prefers_env_override() {
        let temp = tempfile::tempdir().expect("tempdir");
        let marker_path = temp.path().join("git-called");
        let sha = resolve_git_sha(normalize_git_sha(Some(" abc1234 ".to_owned())), || {
            fs::write(&marker_path, b"called").expect("write marker");
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_owned()
        });

        assert_eq!(sha, "abc1234");
        assert!(
            !marker_path.exists(),
            "git shim should not have been called"
        );
    }

    #[test]
    fn check_mode_reuses_existing_manifest_git_sha() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(&manifest_path, r#"{ "git_sha": " existing-sha " }"#).expect("write manifest");

        let sha = existing_manifest_git_sha(&manifest_path).expect("read sha");

        assert_eq!(sha.as_deref(), Some("existing-sha"));
    }
}
