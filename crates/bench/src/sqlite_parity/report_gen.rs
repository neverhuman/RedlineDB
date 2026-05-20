use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::catalog;
use super::cli::{parse_case_list, validate_known_case_ids};

const README_BEGIN: &str = "<!-- sqlite-parity-report:begin -->";
const README_END: &str = "<!-- sqlite-parity-report:end -->";

#[derive(Debug)]
pub struct ReportOptions {
    pub input: PathBuf,
    pub case_list: PathBuf,
    pub out_dir: PathBuf,
    pub readme: PathBuf,
    pub plot: PathBuf,
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
    generated_cases: usize,
    approved_cases: usize,
    remaining_cases: usize,
    coverage_pct: f64,
    ranked_cases: usize,
    measured_samples: usize,
    warmup_samples: usize,
    sqlite_version: String,
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

pub fn generate(options: ReportOptions) -> Result<()> {
    let all_cases = catalog::all_cases()?;
    let approved = parse_case_list(&options.case_list)?;
    validate_known_case_ids(&approved, &all_cases)?;
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read sqlite parity raw input {}", options.input.display()))?;
    let raw_records = parse_raw_records(&raw_text)?;
    let report = build_report(&all_cases, &approved, raw_records, &options.updated_date);

    let raw_out = options.out_dir.join("raw.jsonl");
    let ranked_out = options.out_dir.join("ranked.csv");
    let summary_out = options.out_dir.join("summary.json");
    let manifest_out = options.out_dir.join("manifest.json");

    let ranked_csv = ranked_csv(&report.ranked);
    let summary_json = serde_json::to_string_pretty(&report.summary)? + "\n";
    let svg = latency_svg(&report.ranked, &report.summary);
    let readme = replace_readme_block(
        &fs::read_to_string(&options.readme)
            .with_context(|| format!("read README {}", options.readme.display()))?,
        &readme_block(&report.ranked, &report.summary, &options.plot),
    )?;

    let mut input_hashes = BTreeMap::new();
    input_hashes.insert(
        options.input.display().to_string(),
        sha256_hex(raw_text.as_bytes()),
    );
    input_hashes.insert(
        options.case_list.display().to_string(),
        sha256_file(&options.case_list)?,
    );
    input_hashes.insert(
        "crates/bench/sqlite_parity/generated_manifest.json".to_owned(),
        sha256_hex(include_bytes!(
            "../../sqlite_parity/generated_manifest.json"
        )),
    );

    let mut output_hashes = BTreeMap::new();
    output_hashes.insert("raw.jsonl".to_owned(), sha256_hex(raw_text.as_bytes()));
    output_hashes.insert("ranked.csv".to_owned(), sha256_hex(ranked_csv.as_bytes()));
    output_hashes.insert(
        "summary.json".to_owned(),
        sha256_hex(summary_json.as_bytes()),
    );
    output_hashes.insert(
        options.plot.display().to_string(),
        sha256_hex(svg.as_bytes()),
    );
    output_hashes.insert(
        options.readme.display().to_string(),
        sha256_hex(readme.as_bytes()),
    );
    let manifest = ManifestJson {
        command: normalized_command(&options.command),
        git_sha: git_sha(),
        sqlite_version: report.summary.sqlite_version.clone(),
        updated_date: options.updated_date,
        repetitions: report.repetitions,
        warmup: report.warmup,
        input_hashes,
        output_hashes,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";

    let writes = [
        (raw_out, raw_text),
        (ranked_out, ranked_csv),
        (summary_out, summary_json),
        (manifest_out, manifest_json),
        (options.plot, svg),
        (options.readme, readme),
    ];
    if options.check {
        check_files(&writes)
    } else {
        write_files(&writes)
    }
}

struct BuiltReport {
    ranked: Vec<RankedCase>,
    summary: SummaryJson,
    repetitions: usize,
    warmup: usize,
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
    approved: &BTreeSet<String>,
    raw_records: Vec<RawRecord>,
    updated_date: &str,
) -> BuiltReport {
    let mut grouped = BTreeMap::<String, Vec<RawRecord>>::new();
    let mut sqlite_version = String::from("<unknown>");
    let mut warmup = 0usize;
    let mut measured = 0usize;
    for record in raw_records {
        if sqlite_version == "<unknown>"
            && let Some(version) = &record.sqlite_version
            && !version.is_empty()
        {
            sqlite_version = version.clone();
        }
        if is_warmup(&record) {
            warmup = warmup.saturating_add(1);
            continue;
        }
        if is_measured(&record) {
            measured = measured.saturating_add(1);
            grouped
                .entry(record.case_id.clone())
                .or_default()
                .push(record);
        }
    }

    let case_files = all_cases
        .iter()
        .map(|case| (case.display_id(), case.case_file_name()))
        .collect::<BTreeMap<_, _>>();
    let mut ranked = Vec::new();
    for id in approved {
        let Some(records) = grouped.get(id) else {
            continue;
        };
        let passed = records
            .iter()
            .filter(|record| record.status == "passed")
            .collect::<Vec<_>>();
        if passed.is_empty() {
            continue;
        }
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
            case_files.get(id).cloned().unwrap_or_default()
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
    let approved_cases = approved.len();
    let coverage_pct = approved_cases as f64 / generated_cases.max(1) as f64 * 100.0;
    let repetitions = grouped.values().map(Vec::len).max().unwrap_or(0);
    let warmup_per_case = if approved_cases == 0 {
        0
    } else {
        warmup / approved_cases
    };
    let ranked_cases = ranked.len();
    BuiltReport {
        ranked,
        summary: SummaryJson {
            updated_date: updated_date.to_owned(),
            generated_cases,
            approved_cases,
            remaining_cases: generated_cases.saturating_sub(approved_cases),
            coverage_pct,
            ranked_cases,
            measured_samples: measured,
            warmup_samples: warmup,
            sqlite_version,
        },
        repetitions,
        warmup: warmup_per_case,
    }
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
    (sqlite_median_ns as f64 - redline_median_ns as f64) / sqlite_median_ns.max(1) as f64 * 100.0
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

fn readme_block(ranked: &[RankedCase], summary: &SummaryJson, plot: &Path) -> String {
    let mut out = String::new();
    out.push_str(README_BEGIN);
    out.push('\n');
    out.push_str(&format!(
        "\n**SQLite parity coverage:** **{} / {} = {:.1}%** approved generated cases, with **{}** remaining. Updated {}.\n\n",
        summary.approved_cases,
        summary.generated_cases,
        summary.coverage_pct,
        summary.remaining_cases,
        summary.updated_date
    ));
    out.push_str(&format!(
        "![SQLite parity latency improvement plot]({})\n\n",
        plot.display()
    ));
    out.push_str("<details>\n<summary>Full ranked latency table</summary>\n\n");
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

fn replace_readme_block(readme: &str, block: &str) -> Result<String> {
    if let (Some(begin), Some(end)) = (readme.find(README_BEGIN), readme.find(README_END)) {
        let end = end + README_END.len();
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
<desc id="desc">Median latency improvement vs SQLite. Positive means RedlineDB is faster; negative means regression. Coverage {} of {} approved generated cases with {} measured samples.</desc>
<rect width="1200" height="520" fill="#ffffff"/>
<text x="70" y="34" font-family="sans-serif" font-size="22" font-weight="700">SQLite parity latency improvement vs SQLite, Updated {}</text>
<text x="70" y="60" font-family="sans-serif" font-size="14" fill="#374151">Coverage: {} / {} = {:.1}% approved cases; measured samples: {}; colormap legend: regression red, near-parity neutral, gain green/blue</text>
<line x1="{left}" y1="{zero_y:.2}" x2="1160" y2="{zero_y:.2}" stroke="#111827" stroke-width="2"/>
<text x="74" y="{:.2}" font-family="sans-serif" font-size="12" fill="#111827">0% horizontal reference line</text>
<line x1="{left}" y1="{top}" x2="{left}" y2="448" stroke="#4b5563"/>
<line x1="{left}" y1="448" x2="1160" y2="448" stroke="#4b5563"/>
<text x="570" y="498" font-family="sans-serif" font-size="13" text-anchor="middle">Ranked approved tests, worst RedlineDB gap to largest gain</text>
<text x="18" y="270" font-family="sans-serif" font-size="13" transform="rotate(-90 18 270)" text-anchor="middle">Median latency improvement vs SQLite (%)</text>
"##,
        summary.updated_date,
        summary.approved_cases,
        summary.generated_cases,
        summary.measured_samples,
        summary.updated_date,
        summary.approved_cases,
        summary.generated_cases,
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

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("hash {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git_sha() -> String {
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

    #[test]
    fn improvement_sign_convention_and_ranking() {
        let mut ranked = vec![
            RankedCase {
                improvement_pct: improvement_pct(100, 200),
                case_id: "00001".to_owned(),
                name: "regression".to_owned(),
                case_file: "a.rs".to_owned(),
                priority: "P0".to_owned(),
                profile: "memory".to_owned(),
                category: "fixture".to_owned(),
                sqlite_median_ns: 100,
                redline_median_ns: 200,
                samples: 1,
            },
            RankedCase {
                improvement_pct: improvement_pct(100, 50),
                case_id: "00002".to_owned(),
                name: "gain".to_owned(),
                case_file: "b.rs".to_owned(),
                priority: "P0".to_owned(),
                profile: "memory".to_owned(),
                category: "fixture".to_owned(),
                sqlite_median_ns: 100,
                redline_median_ns: 50,
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
        let approved = BTreeSet::from(["00001".to_owned()]);
        let report = build_report(&all_cases, &approved, records, "2026-05-20");
        assert_eq!(report.ranked[0].sqlite_median_ns, 300);
        assert_eq!(report.ranked[0].redline_median_ns, 120);
        assert_eq!(report.summary.warmup_samples, 1);
    }

    #[test]
    fn readme_marker_replacement_preserves_surrounding_content() {
        let current = format!("before\n{README_BEGIN}\nold\n{README_END}\nafter\n");
        let next = replace_readme_block(&current, "new block\n").expect("replace");
        assert_eq!(next, "before\nnew block\nafter\n");
    }

    #[test]
    fn svg_contains_required_labels() {
        let ranked = vec![RankedCase {
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
        }];
        let summary = SummaryJson {
            updated_date: "2026-05-20".to_owned(),
            generated_cases: 1127,
            approved_cases: 612,
            remaining_cases: 515,
            coverage_pct: 54.30346,
            ranked_cases: 1,
            measured_samples: 1,
            warmup_samples: 0,
            sqlite_version: "3.fixture".to_owned(),
        };
        let svg = latency_svg(&ranked, &summary);
        assert!(svg.contains("Updated 2026-05-20"));
        assert!(svg.contains("Median latency improvement vs SQLite (%)"));
        assert!(svg.contains("colormap legend"));
        assert!(svg.contains("0% horizontal reference line"));
    }
}
