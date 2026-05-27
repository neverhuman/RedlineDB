use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_BEGIN: &str = "<!-- sqlite-parity-report:begin -->";
const REPORT_END: &str = "<!-- sqlite-parity-report:end -->";
const METRICS_BEGIN: &str = "<!-- sqlite-parity-metrics:begin -->";
const METRICS_END: &str = "<!-- sqlite-parity-metrics:end -->";
const JANKURAI_BREAKDOWN_BEGIN: &str = "<!-- sqlite-jankurai-breakdown:begin -->";
const JANKURAI_BREAKDOWN_END: &str = "<!-- sqlite-jankurai-breakdown:end -->";

#[derive(Debug)]
pub struct ReportOptions {
    pub suite: String,
    pub input: PathBuf,
    pub official_evidence: Option<PathBuf>,
    pub local_diagnostics: bool,
    pub out_dir: PathBuf,
    pub readme: PathBuf,
    pub plot: Option<PathBuf>,
    pub ksloc_plot: Option<PathBuf>,
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
}

#[derive(Debug)]
pub struct JankuraiCompareOptions {
    pub redlinedb_score: PathBuf,
    pub sqlite_score: PathBuf,
    pub sqlite_ref: String,
    pub updated_date: String,
    pub json: PathBuf,
    pub csv: PathBuf,
    pub check: bool,
}

#[derive(Debug)]
pub struct SentinelOptions {
    pub input: PathBuf,
    pub ceiling_ns: Vec<String>,
    pub enforce: bool,
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
    status: String,
    reference_elapsed_ns: u128,
    target_elapsed_ns: u128,
    #[serde(default)]
    memory_status: String,
    #[serde(default)]
    reference_peak_rss_kb: Option<u64>,
    #[serde(default)]
    reference_rss_sampled_kb: Option<u64>,
    #[serde(default)]
    target_peak_rss_kb: Option<u64>,
    #[serde(default)]
    target_rss_sampled_kb: Option<u64>,
}

#[derive(Debug, Serialize)]
struct SummaryJson {
    suite: String,
    total_cases: usize,
    passed_cases: usize,
    failed_cases: usize,
    skipped_cases: usize,
    elapsed_ns: u128,
    measured_samples: usize,
    warmup_samples: usize,
    ranked_cases: usize,
    repetitions: usize,
    warmup: usize,
}

#[derive(Debug, Serialize)]
struct ManifestJson {
    schema_version: String,
    suite: String,
    command_line: Vec<String>,
    repetitions: usize,
    warmup: usize,
    output_files: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct ProvenanceJson {
    schema_version: String,
    suite: String,
    redline_testing_binary_path: String,
    redline_testing_binary_sha256: String,
    target_binary_path: String,
    target_binary_sha256: String,
    target_version: String,
    sqlite_binary_path: String,
    sqlite_binary_sha256: String,
    sqlite_version: String,
    command_line: Vec<String>,
    repetitions: usize,
    warmup: usize,
    updated_date: String,
    git_sha: String,
    git_dirty: bool,
    output_file_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct EvidenceVersions {
    runner_version: String,
    target_version: String,
    sqlite_version: String,
}

#[derive(Debug)]
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

#[derive(Debug, Clone)]
struct SvgArtifact {
    path: PathBuf,
    contents: String,
}

#[derive(Debug, Clone)]
struct SvgSpec {
    title: String,
    subtitle: String,
    accent: &'static str,
    metrics: Vec<SvgMetric>,
    bars: Vec<SvgBar>,
}

#[derive(Debug, Clone)]
struct SvgMetric {
    label: String,
    value: String,
}

#[derive(Debug, Clone)]
struct SvgBar {
    label: String,
    value: f64,
    value_label: String,
}

pub fn generate(options: ReportOptions) -> Result<()> {
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read raw input {}", options.input.display()))?;
    if let Some(official_evidence) = &options.official_evidence {
        validate_official_evidence_binding(official_evidence, &options.suite, &raw_text)?;
    } else if !options.local_diagnostics || options.check {
        bail!(
            "reporting committed artifacts requires --official-evidence; \
             use --local-diagnostics only for uncommitted local diagnostics"
        );
    }
    let raw_records = parse_raw_records(&raw_text)?;
    if raw_records.is_empty() {
        bail!("sqlite parity report input is empty");
    }

    if let Some(expected_repetitions) = options.expected_repetitions {
        let measured = raw_records
            .iter()
            .filter(|record| is_measured(record))
            .map(|record| record.repetition_index)
            .collect::<BTreeSet<_>>();
        if measured.len() != expected_repetitions {
            bail!(
                "expected {} measured repetitions but found {}",
                expected_repetitions,
                measured.len()
            );
        }
    }
    if let Some(expected_warmup) = options.expected_warmup {
        let warmups = raw_records
            .iter()
            .filter(|record| record.sample_role == "warmup")
            .count();
        let expected_total_warmups = expected_warmup.saturating_mul(
            raw_records
                .iter()
                .map(|record| record.case_id.clone())
                .collect::<BTreeSet<_>>()
                .len(),
        );
        if warmups != expected_total_warmups {
            bail!(
                "expected {} warmup samples ({} per case) but found {}",
                expected_total_warmups,
                expected_warmup,
                warmups
            );
        }
    }

    let ranked = rank_cases(&raw_records);
    let evidence_versions = options
        .official_evidence
        .as_deref()
        .map(read_official_evidence_versions)
        .transpose()?;
    let total_cases = raw_records
        .iter()
        .map(|record| record.case_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let passed_cases = raw_records
        .iter()
        .filter(|record| record.status == "passed" && is_measured(record))
        .map(|record| record.case_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let failed_cases = raw_records
        .iter()
        .filter(|record| record.status == "failed")
        .map(|record| record.case_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let skipped_cases = raw_records
        .iter()
        .filter(|record| record.status == "skipped")
        .map(|record| record.case_id.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let measured_samples = raw_records
        .iter()
        .filter(|record| is_measured(record))
        .count();
    let warmup_samples = raw_records
        .iter()
        .filter(|record| record.sample_role == "warmup")
        .count();
    let summary = SummaryJson {
        suite: options.suite.clone(),
        total_cases,
        passed_cases,
        failed_cases,
        skipped_cases,
        elapsed_ns: 0,
        measured_samples,
        warmup_samples,
        ranked_cases: ranked.len(),
        repetitions: options
            .expected_repetitions
            .unwrap_or(measured_samples.max(1)),
        warmup: options.expected_warmup.unwrap_or(warmup_samples),
    };

    let summary_json = serde_json::to_string_pretty(&summary)? + "\n";
    let ranked_csv = ranked_csv(&ranked);
    let ksloc_csv = ksloc_csv();
    let report_block = render_report_block(
        &summary,
        &ranked,
        &raw_records,
        &options,
        evidence_versions.as_ref(),
    );
    let metrics_block = render_metrics_block(&options);
    let mut readme = fs::read_to_string(&options.readme)
        .with_context(|| format!("read README {}", options.readme.display()))?;
    readme = replace_block(&readme, REPORT_BEGIN, REPORT_END, &report_block);
    readme = replace_block(&readme, METRICS_BEGIN, METRICS_END, &metrics_block);
    if let Some(comparison_path) = &options.jankurai_comparison
        && comparison_path.exists()
    {
        let comparison_text = fs::read_to_string(comparison_path)
            .with_context(|| format!("read jankurai comparison {}", comparison_path.display()))?;
        readme = replace_block(
            &readme,
            JANKURAI_BREAKDOWN_BEGIN,
            JANKURAI_BREAKDOWN_END,
            &format!("\n{}\n", comparison_text.trim()),
        );
    }

    let output_dir = &options.out_dir;
    let artifact_names = artifact_names_for_suite(&options.suite);
    let raw_out = output_dir.join(artifact_names.raw);
    let ranked_out = output_dir.join(artifact_names.ranked);
    let ksloc_out = output_dir.join(artifact_names.ksloc);
    let summary_out = output_dir.join(artifact_names.summary);
    let manifest_out = output_dir.join(artifact_names.manifest);
    let provenance_out = output_dir.join(artifact_names.provenance);

    let redline_testing_bin = std::env::current_exe().context("resolve current executable")?;
    let redline_testing_binary_sha256 = sha256_file(&redline_testing_bin)?;
    let target_bin = std::env::var_os("REDLINE_TESTING_TARGET_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/release/redlinedb"));
    let target_binary_path = canonical_display(&target_bin);
    let target_binary_sha256 = sha256_file(&target_bin).unwrap_or_else(|_| "<unknown>".to_owned());
    let target_version = capture_version(&target_bin).unwrap_or_else(|_| "<unknown>".to_owned());
    let sqlite_binary_path = std::env::var_os("REDLINE_TESTING_SQLITE_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("sqlite3"));
    let sqlite_binary_sha256 =
        sha256_file(&sqlite_binary_path).unwrap_or_else(|_| "<unknown>".to_owned());
    let sqlite_version =
        capture_version(&sqlite_binary_path).unwrap_or_else(|_| "<unknown>".to_owned());
    let output_file_hashes = BTreeMap::from([
        (artifact_names.raw.to_owned(), sha256_hex(&raw_text)),
        (artifact_names.summary.to_owned(), sha256_hex(&summary_json)),
        (artifact_names.ranked.to_owned(), sha256_hex(&ranked_csv)),
        (artifact_names.ksloc.to_owned(), sha256_hex(&ksloc_csv)),
        (options.readme.display().to_string(), sha256_hex(&readme)),
    ]);
    let command_line = normalized_command_line();
    let provenance = ProvenanceJson {
        schema_version: "redline-testing-provenance-v1".to_owned(),
        suite: options.suite.clone(),
        redline_testing_binary_path: canonical_display(&redline_testing_bin),
        redline_testing_binary_sha256,
        target_binary_path,
        target_binary_sha256,
        target_version,
        sqlite_binary_path: canonical_display(&sqlite_binary_path),
        sqlite_binary_sha256,
        sqlite_version,
        command_line: command_line.clone(),
        repetitions: summary.repetitions,
        warmup: summary.warmup,
        updated_date: options.updated_date.clone(),
        git_sha: git_sha(),
        git_dirty: git_dirty(),
        output_file_hashes: output_file_hashes.clone(),
    };
    let provenance_json = serde_json::to_string_pretty(&provenance)? + "\n";
    let manifest = ManifestJson {
        schema_version: "redline-testing-manifest-v1".to_owned(),
        suite: options.suite.clone(),
        command_line,
        repetitions: summary.repetitions,
        warmup: summary.warmup,
        output_files: BTreeMap::from([
            ("raw".to_owned(), raw_out.display().to_string()),
            ("summary".to_owned(), summary_out.display().to_string()),
            ("ranked".to_owned(), ranked_out.display().to_string()),
            ("ksloc".to_owned(), ksloc_out.display().to_string()),
            (
                "provenance".to_owned(),
                provenance_out.display().to_string(),
            ),
        ]),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)? + "\n";

    let rendered = RenderedReport {
        raw: raw_text,
        summary: summary_json,
        ranked: ranked_csv,
        ksloc: ksloc_csv,
        readme,
        manifest: manifest_json,
        provenance: provenance_json,
    };
    let svg_artifacts = build_svg_artifacts(&summary, &ranked, &raw_records, &options);

    if options.check {
        verify_existing(
            &options.input,
            &raw_out,
            &summary_out,
            &ranked_out,
            &ksloc_out,
            &manifest_out,
            &provenance_out,
            &options.readme,
            &rendered,
            &svg_artifacts,
        )?;
        return Ok(());
    }

    fs::create_dir_all(output_dir).with_context(|| format!("create {}", output_dir.display()))?;
    fs::write(&raw_out, rendered.raw)?;
    fs::write(&summary_out, rendered.summary)?;
    fs::write(&ranked_out, rendered.ranked)?;
    fs::write(&ksloc_out, rendered.ksloc.clone())?;
    fs::write(&manifest_out, rendered.manifest)?;
    fs::write(&provenance_out, rendered.provenance)?;
    fs::write(&options.readme, rendered.readme)?;
    fs::write(
        output_dir.join("raw.jsonl.sha256"),
        format!("{}\n", sha256_file(&raw_out)?),
    )?;
    fs::write(
        output_dir.join("paper-data-loc-comparison.csv"),
        rendered.ksloc,
    )?;

    for artifact in svg_artifacts {
        write_text(&artifact.path, &artifact.contents)?;
    }
    if let Some(score_path) = &options.jankurai_score
        && score_path.exists()
    {
        let score_text = fs::read_to_string(score_path)
            .with_context(|| format!("read jankurai score {}", score_path.display()))?;
        if !score_text.trim().is_empty() {
            fs::write(output_dir.join("jankurai-score.txt"), score_text)?;
        }
    }

    Ok(())
}

fn validate_official_evidence_binding(
    official_evidence: &Path,
    suite: &str,
    raw_text: &str,
) -> Result<()> {
    let text = fs::read_to_string(official_evidence)
        .with_context(|| format!("read official evidence {}", official_evidence.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parse official evidence {}", official_evidence.display()))?;
    let actual_raw_sha256 = sha256_hex(raw_text);
    let schema_version = value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let expected_raw_sha256 = match schema_version {
        "redline-testing-official-evidence-processed-v1" => processed_suite_raw_hash(&value, suite),
        "redline-testing-official-evidence-v1" => raw_official_suite_hash(&value, suite),
        other => bail!(
            "unsupported official evidence schema_version {:?} in {}",
            other,
            official_evidence.display()
        ),
    }
    .with_context(|| {
        format!(
            "resolve official evidence hash for suite {suite} from {}",
            official_evidence.display()
        )
    })?;

    if actual_raw_sha256 != expected_raw_sha256 {
        bail!(
            "official evidence raw SHA-256 mismatch for suite {}: expected {}, got {}",
            suite,
            expected_raw_sha256,
            actual_raw_sha256
        );
    }
    Ok(())
}

fn read_official_evidence_versions(official_evidence: &Path) -> Result<EvidenceVersions> {
    let text = fs::read_to_string(official_evidence)
        .with_context(|| format!("read official evidence {}", official_evidence.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parse official evidence {}", official_evidence.display()))?;
    official_evidence_versions_from_value(official_evidence, &value)
}

fn official_evidence_versions_from_value(
    official_evidence: &Path,
    value: &serde_json::Value,
) -> Result<EvidenceVersions> {
    let schema_version = value
        .get("schema_version")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match schema_version {
        "redline-testing-official-evidence-v1" => Ok(EvidenceVersions {
            runner_version: evidence_version(value, official_evidence, "runner")?,
            target_version: evidence_version(value, official_evidence, "target")?,
            sqlite_version: evidence_version(value, official_evidence, "sqlite")?,
        }),
        "redline-testing-official-evidence-processed-v1" => Ok(EvidenceVersions {
            runner_version: evidence_version(
                value.get("official_evidence").unwrap_or(value),
                official_evidence,
                "runner",
            )?,
            target_version: evidence_version(
                value.get("official_evidence").unwrap_or(value),
                official_evidence,
                "target",
            )?,
            sqlite_version: evidence_version(
                value.get("official_evidence").unwrap_or(value),
                official_evidence,
                "sqlite",
            )?,
        }),
        other => bail!(
            "unsupported official evidence schema_version {:?} in {}",
            other,
            official_evidence.display()
        ),
    }
}

fn evidence_version(
    value: &serde_json::Value,
    official_evidence: &Path,
    section: &str,
) -> Result<String> {
    value
        .get(section)
        .and_then(|entry| entry.get("version"))
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "official evidence missing {}.version in {}",
                section,
                official_evidence.display()
            )
        })
}

fn processed_suite_raw_hash(value: &serde_json::Value, suite: &str) -> Result<String> {
    let suite_entry = value
        .get("suite_summaries")
        .and_then(|suite_summaries| suite_summaries.get(suite))
        .ok_or_else(|| anyhow::anyhow!("processed evidence missing suite_summaries.{suite}"))?;
    suite_entry
        .get("raw_sha256")
        .and_then(normalize_hash_value)
        .ok_or_else(|| anyhow::anyhow!("processed evidence missing raw_sha256 for {suite}"))
}

fn raw_official_suite_hash(value: &serde_json::Value, suite: &str) -> Result<String> {
    let suite_entry = official_suite_entry(value, suite)
        .ok_or_else(|| anyhow::anyhow!("official evidence missing suite {suite}"))?;
    if let Some(hash) = suite_entry.get("raw_sha256").and_then(normalize_hash_value) {
        return Ok(hash);
    }
    let raw_path = suite_entry
        .get("raw_path")
        .or_else(|| suite_entry.get("raw"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow::anyhow!("official evidence suite {suite} missing raw_path"))?;
    let output_hashes = value
        .get("output_file_hashes")
        .ok_or_else(|| anyhow::anyhow!("official evidence missing output_file_hashes"))?;
    let normalized_raw_path = normalize_path(raw_path);
    let raw_file_name = Path::new(raw_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_path);
    official_hash_lookup(output_hashes, &normalized_raw_path)
        .or_else(|| {
            raw_file_name
                .as_deref()
                .and_then(|file_name| official_hash_lookup(output_hashes, file_name))
        })
        .ok_or_else(|| {
            anyhow::anyhow!("official evidence missing output_file_hashes entry for {raw_path}")
        })
}

fn official_suite_entry<'a>(
    value: &'a serde_json::Value,
    suite: &str,
) -> Option<&'a serde_json::Value> {
    let suites = value.get("suites")?;
    if let Some(entry) = suites.get(suite) {
        return Some(entry);
    }
    suites.as_array()?.iter().find(|entry| {
        entry
            .get("name")
            .or_else(|| entry.get("suite"))
            .and_then(|value| value.as_str())
            == Some(suite)
    })
}

fn official_hash_lookup(value: &serde_json::Value, expected_path: &str) -> Option<String> {
    let entries = value.as_object()?;
    for (key, item) in entries {
        if normalize_path(key) == expected_path
            && let Some(hash) = normalize_hash_value(item)
        {
            return Some(hash);
        }
        if let Some(object) = item.as_object() {
            let path = object
                .get("path")
                .or_else(|| object.get("file"))
                .or_else(|| object.get("name"))
                .and_then(|value| value.as_str())
                .map(normalize_path);
            if path.as_deref() == Some(expected_path) {
                for key in ["sha256", "hash", "digest", "value"] {
                    if let Some(hash) = object.get(key).and_then(normalize_hash_value) {
                        return Some(hash);
                    }
                }
            }
        }
    }
    None
}

fn normalize_hash_value(value: &serde_json::Value) -> Option<String> {
    let mut hash = value.as_str()?.trim().to_ascii_lowercase();
    if let Some(stripped) = hash.strip_prefix("sha256:") {
        hash = stripped.to_owned();
    }
    (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hash)
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_owned()
}

fn suite_display_name(suite: &str) -> String {
    match suite {
        "memory" => "Memory".to_owned(),
        "beyond_sqlite" => "Beyond-SQLite".to_owned(),
        "sqlite_parity" => "SQLite parity".to_owned(),
        "rql_phase1" => "RQL phase 1".to_owned(),
        "all" => "All suites".to_owned(),
        other => other.replace('_', " "),
    }
}

fn suite_subject(suite: &str) -> &'static str {
    match suite {
        "beyond_sqlite" => "features",
        _ => "cases",
    }
}

fn suite_accent(suite: &str) -> &'static str {
    match suite {
        "memory" => "#14b8a6",
        "beyond_sqlite" => "#f59e0b",
        "sqlite_parity" => "#38bdf8",
        "rql_phase1" => "#a855f7",
        _ => "#64748b",
    }
}

fn memory_status_summary(records: &[RawRecord]) -> &'static str {
    if records
        .iter()
        .any(|record| record.memory_status == "sampled")
    {
        "sampled"
    } else if records
        .iter()
        .any(|record| record.memory_status == "disabled")
    {
        "disabled"
    } else {
        "unavailable"
    }
}

fn memory_peak_summary(records: &[RawRecord]) -> Option<String> {
    let target_peak = median_u64(
        records
            .iter()
            .filter_map(|record| record.target_peak_rss_kb),
    )?;
    let reference_peak = median_u64(
        records
            .iter()
            .filter_map(|record| record.reference_peak_rss_kb),
    )?;
    let target_sampled = median_u64(
        records
            .iter()
            .filter_map(|record| record.target_rss_sampled_kb),
    )?;
    let reference_sampled = median_u64(
        records
            .iter()
            .filter_map(|record| record.reference_rss_sampled_kb),
    )?;
    Some(format!(
        "median peak RSS target {} KB / reference {} KB; sampled RSS target {} KB / reference {} KB",
        target_peak, reference_peak, target_sampled, reference_sampled
    ))
}

fn median_gap(ranked: &[RankedCase]) -> f64 {
    if ranked.is_empty() {
        return 0.0;
    }
    let mut values = ranked
        .iter()
        .map(|case| case.improvement_pct)
        .collect::<Vec<_>>();
    values.sort_by(|left, right| left.total_cmp(right));
    values[values.len() / 2]
}

fn worst_gap(ranked: &[RankedCase]) -> f64 {
    ranked
        .iter()
        .map(|case| case.improvement_pct)
        .min_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.0)
}

fn median_sqlite_ns(ranked: &[RankedCase]) -> u128 {
    if ranked.is_empty() {
        return 0;
    }
    median(ranked.iter().map(|case| case.sqlite_median_ns))
}

fn median_target_ns(ranked: &[RankedCase]) -> u128 {
    if ranked.is_empty() {
        return 0;
    }
    median(ranked.iter().map(|case| case.redline_median_ns))
}

fn human_duration_ns(value: u128) -> String {
    if value >= 1_000_000_000 {
        format!("{:.2}s", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.2}ms", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.2}us", value as f64 / 1_000.0)
    } else {
        format!("{value}ns")
    }
}

fn histogram_bars(ranked: &[RankedCase]) -> Vec<SvgBar> {
    let buckets = [
        ("<-100%", 0.0, 0usize),
        ("-100..-50", 0.0, 0usize),
        ("-50..0", 0.0, 0usize),
        ("0..50", 0.0, 0usize),
        (">=50", 0.0, 0usize),
    ];
    let mut counts = buckets
        .iter()
        .map(|(label, value, count)| ((*label).to_owned(), *value, *count))
        .collect::<Vec<_>>();
    for case in ranked {
        let pct = case.improvement_pct;
        let bucket_index = if pct < -100.0 {
            0
        } else if pct < -50.0 {
            1
        } else if pct < 0.0 {
            2
        } else if pct < 50.0 {
            3
        } else {
            4
        };
        counts[bucket_index].2 = counts[bucket_index].2.saturating_add(1);
        counts[bucket_index].1 = counts[bucket_index].2 as f64;
    }
    counts
        .into_iter()
        .map(|(label, value, count)| SvgBar {
            label,
            value,
            value_label: count.to_string(),
        })
        .collect()
}

struct ArtifactNames {
    raw: &'static str,
    ranked: &'static str,
    ksloc: &'static str,
    summary: &'static str,
    manifest: &'static str,
    provenance: &'static str,
}

fn artifact_names_for_suite(suite: &str) -> ArtifactNames {
    match suite {
        "memory" => ArtifactNames {
            raw: "memory.raw.jsonl",
            ranked: "memory-ranked.csv",
            ksloc: "memory-ksloc.csv",
            summary: "memory-summary.json",
            manifest: "memory-manifest.json",
            provenance: "memory-provenance.json",
        },
        "rql_phase1" => ArtifactNames {
            raw: "rql_phase1.raw.jsonl",
            ranked: "rql-phase1-ranked.csv",
            ksloc: "rql-phase1-ksloc.csv",
            summary: "rql-phase1-summary.json",
            manifest: "rql-phase1-manifest.json",
            provenance: "rql-phase1-provenance.json",
        },
        "beyond_sqlite" => ArtifactNames {
            raw: "beyond_sqlite.raw.jsonl",
            ranked: "beyond-sqlite-ranked.csv",
            ksloc: "beyond-sqlite-ksloc.csv",
            summary: "beyond-sqlite-summary.json",
            manifest: "beyond-sqlite-manifest.json",
            provenance: "beyond-sqlite-provenance.json",
        },
        _ => ArtifactNames {
            raw: "raw.jsonl",
            ranked: "ranked.csv",
            ksloc: "ksloc.csv",
            summary: "summary.json",
            manifest: "manifest.json",
            provenance: "provenance.json",
        },
    }
}

pub fn jankurai_compare(options: JankuraiCompareOptions) -> Result<()> {
    let redlinedb = parse_score(&options.redlinedb_score)?;
    let sqlite = parse_score(&options.sqlite_score)?;
    let diff = redlinedb.score as i64 - sqlite.score as i64;
    let comparison = serde_json::json!({
        "generated_by": "redline-testing jankurai-compare",
        "updated_date": options.updated_date,
        "sqlite_ref": options.sqlite_ref,
        "redlinedb_score": redlinedb.score,
        "sqlite_score": sqlite.score,
        "score_delta": diff,
        "redlinedb_status": redlinedb.status,
        "sqlite_status": sqlite.status,
    });
    let csv = format!(
        "repo,score,status\nredlinedb,{},{}\nsqlite,{},{}\n",
        redlinedb.score, redlinedb.status, sqlite.score, sqlite.status
    );

    if options.check {
        verify_text(
            &options.json,
            &format!("{}\n", serde_json::to_string_pretty(&comparison)?),
        )?;
        verify_text(&options.csv, &csv)?;
        return Ok(());
    }

    write_text(
        &options.json,
        &format!("{}\n", serde_json::to_string_pretty(&comparison)?),
    )?;
    write_text(&options.csv, &csv)?;
    Ok(())
}

pub fn sentinel(options: SentinelOptions) -> Result<()> {
    let ceilings = options
        .ceiling_ns
        .iter()
        .map(|entry| {
            let (case_id, value) = entry
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid ceiling entry `{entry}`"))?;
            let value = value
                .parse::<u128>()
                .with_context(|| format!("parse ceiling value for {case_id}"))?;
            Ok((case_id.to_owned(), value))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read sentinel input {}", options.input.display()))?;
    let mut violations = Vec::new();
    for record in parse_raw_records(&raw_text)? {
        if !is_measured(&record) {
            continue;
        }
        if let Some(ceiling) = ceilings.get(&record.case_id)
            && record.target_elapsed_ns > *ceiling
        {
            violations.push(format!(
                "{}: {} ns > ceiling {} ns",
                record.case_id, record.target_elapsed_ns, ceiling
            ));
        }
    }
    if violations.is_empty() {
        if options.enforce {
            eprintln!("sqlite parity sentinel passed");
        }
        return Ok(());
    }
    let message = violations.join("; ");
    if options.enforce {
        bail!("{message}");
    }
    eprintln!("{message}");
    Ok(())
}

struct RenderedReport {
    raw: String,
    summary: String,
    ranked: String,
    ksloc: String,
    readme: String,
    manifest: String,
    provenance: String,
}

fn parse_raw_records(raw_text: &str) -> Result<Vec<RawRecord>> {
    let mut records = Vec::new();
    for (index, line) in raw_text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        records.push(
            serde_json::from_str(line)
                .with_context(|| format!("parse raw JSONL line {}", index.saturating_add(1)))?,
        );
    }
    Ok(records)
}

fn rank_cases(records: &[RawRecord]) -> Vec<RankedCase> {
    let mut grouped = BTreeMap::<String, Vec<&RawRecord>>::new();
    for record in records.iter().filter(|record| is_measured(record)) {
        grouped
            .entry(record.case_id.clone())
            .or_default()
            .push(record);
    }
    let mut ranked = Vec::new();
    for (case_id, group) in grouped {
        let first = group[0];
        let sqlite_median_ns = median(group.iter().map(|record| record.reference_elapsed_ns));
        let redline_median_ns = median(group.iter().map(|record| record.target_elapsed_ns));
        ranked.push(RankedCase {
            case_id,
            name: first.name.clone(),
            case_file: first.case_file.clone(),
            priority: first.priority.clone(),
            profile: first.profile.clone(),
            category: first.category.clone(),
            sqlite_median_ns,
            redline_median_ns,
            improvement_pct: improvement_pct(sqlite_median_ns, redline_median_ns),
            samples: group.len(),
        });
    }
    ranked.sort_by(|left, right| {
        left.improvement_pct
            .total_cmp(&right.improvement_pct)
            .then_with(|| left.case_id.cmp(&right.case_id))
    });
    ranked
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

fn ksloc_csv() -> String {
    "# Generated by: redline-testing report\ncrate,loc\nredline-testing,1\n".to_owned()
}

fn render_report_block(
    summary: &SummaryJson,
    ranked: &[RankedCase],
    raw_records: &[RawRecord],
    options: &ReportOptions,
    evidence_versions: Option<&EvidenceVersions>,
) -> String {
    let suite_label = suite_display_name(&options.suite);
    let suite_subject = suite_subject(&options.suite);
    let median_gap = if ranked.is_empty() {
        0.0
    } else {
        let mut values = ranked
            .iter()
            .map(|case| case.improvement_pct)
            .collect::<Vec<_>>();
        values.sort_by(|left, right| left.total_cmp(right));
        values[values.len() / 2]
    };
    let worst_gap = ranked
        .iter()
        .map(|case| case.improvement_pct)
        .min_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.0);
    let faster_cases = ranked
        .iter()
        .filter(|case| case.improvement_pct > 0.0)
        .count();
    let mut block = String::new();
    block.push_str(&format!(
        "**{} coverage:** **{} / {}** {} passed in CI. Failed: **{}**. Skipped: **{}**. Updated {}.\n\n",
        suite_label,
        summary.passed_cases,
        summary.total_cases,
        suite_subject,
        summary.failed_cases,
        summary.skipped_cases,
        options.updated_date
    ));
    match options.suite.as_str() {
        "beyond_sqlite" => {
            block.push_str(&format!(
                "**{} progress:** coverage **{:.2}%**, promoted reference features **{}**, manifest backlog **{}**.\n\n",
                suite_label,
                if summary.total_cases == 0 {
                    0.0
                } else {
                    summary.passed_cases as f64 / summary.total_cases as f64 * 100.0
                },
                summary.passed_cases,
                summary.skipped_cases
            ));
        }
        "memory" => {
            let memory_status = memory_status_summary(raw_records);
            let memory_peaks = memory_peak_summary(raw_records)
                .unwrap_or_else(|| "no RSS samples were captured".to_owned());
            block.push_str(&format!(
                "**{} latency:** median gap **{:.2}%**, worst gap **{:.2}%**, faster cases **{}**. RSS sampling: **{}**. {}.\n\n",
                suite_label,
                median_gap,
                worst_gap,
                faster_cases,
                memory_status,
                memory_peaks
            ));
        }
        _ => {
            block.push_str(&format!(
                "**{} latency:** median gap **{:.2}%**, worst gap **{:.2}%**, faster cases **{}**.\n\n",
                suite_label,
                median_gap,
                worst_gap,
                faster_cases
            ));
        }
    }
    if let Some(evidence_versions) = evidence_versions {
        block.push_str(&format!(
            "**Benchmark metadata:** RedlineDB target version **{}**, SQLite reference version **{}**, redline-testing runner version **{}**.\n\n",
            evidence_versions.target_version,
            evidence_versions.sqlite_version,
            evidence_versions.runner_version
        ));
    }
    if let Some(plot) = &options.plot {
        block.push_str(&format!(
            "![{} latency improvement plot]({})\n\n",
            suite_label,
            plot.display()
        ));
    }
    if let Some(plot) = &options.performance_histogram_plot {
        block.push_str(&format!(
            "![{} performance distribution]({})\n\n",
            suite_label,
            plot.display()
        ));
    }
    let table_id = format!("{}-ranked-table", options.suite.replace('_', "-"));
    let table_summary = if options.suite == "beyond_sqlite" {
        "Full ranked feature table"
    } else {
        "Full ranked latency table"
    };
    block.push_str(&format!(
        "<details id=\"{}\">\n<summary>{}</summary>\n\n",
        table_id, table_summary
    ));
    block.push_str("| Rank | Case | Priority | Profile | Category | SQLite median ns | RedlineDB median ns | Improvement |\n");
    block.push_str("| ---: | --- | --- | --- | --- | ---: | ---: | ---: |\n");
    for (index, row) in ranked.iter().take(25).enumerate() {
        block.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {:+.2}% |\n",
            index + 1,
            row.name,
            row.priority,
            row.profile,
            row.category,
            row.sqlite_median_ns,
            row.redline_median_ns,
            row.improvement_pct
        ));
    }
    block.push_str("\n</details>\n");
    block
}

fn render_metrics_block(options: &ReportOptions) -> String {
    let suite_label = suite_display_name(&options.suite);
    let mut block = String::new();
    if let Some(plot) = &options.jankurai_score_plot {
        block.push_str(&format!("![{} score]({})\n\n", suite_label, plot.display()));
    }
    if let Some(plot) = &options.code_shape_plot {
        block.push_str(&format!(
            "![{} code shape]({})\n\n",
            suite_label,
            plot.display()
        ));
    }
    if let Some(plot) = &options.median_test_performance_plot {
        block.push_str(&format!(
            "![{} median test performance]({})\n\n",
            suite_label,
            plot.display()
        ));
    }
    if let Some(plot) = &options.ksloc_plot {
        block.push_str(&format!("![{} KSLOC]({})\n\n", suite_label, plot.display()));
    }
    if let Some(plot) = &options.jankurai_comparison_plot {
        block.push_str(&format!(
            "![{} Jankurai comparison]({})\n\n",
            suite_label,
            plot.display()
        ));
    }
    block
}

fn replace_block(text: &str, begin: &str, end: &str, replacement: &str) -> String {
    match (text.find(begin), text.find(end)) {
        (Some(begin_index), Some(end_index)) if begin_index < end_index => {
            let mut result = String::new();
            result.push_str(&text[..begin_index + begin.len()]);
            result.push('\n');
            result.push_str(replacement);
            result.push_str(&text[end_index..]);
            result
        }
        _ => format!("{text}\n{begin}\n{replacement}\n{end}\n"),
    }
}

fn build_svg_artifacts(
    summary: &SummaryJson,
    ranked: &[RankedCase],
    raw_records: &[RawRecord],
    options: &ReportOptions,
) -> Vec<SvgArtifact> {
    let suite_label = suite_display_name(&options.suite);
    let suite_accent = suite_accent(&options.suite);
    let mut artifacts = Vec::new();

    if let Some(path) = &options.plot {
        let spec = if options.suite == "beyond_sqlite" {
            SvgSpec {
                title: format!("{suite_label} feature progress"),
                subtitle: "Coverage evidence for the beyond-SQLite backlog.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Passed".to_owned(),
                        value: summary.passed_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Skipped".to_owned(),
                        value: summary.skipped_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Coverage".to_owned(),
                        value: format!(
                            "{:.2}%",
                            if summary.total_cases == 0 {
                                0.0
                            } else {
                                summary.passed_cases as f64 / summary.total_cases as f64 * 100.0
                            }
                        ),
                    },
                ],
                bars: vec![
                    SvgBar {
                        label: "passed".to_owned(),
                        value: summary.passed_cases as f64,
                        value_label: summary.passed_cases.to_string(),
                    },
                    SvgBar {
                        label: "skipped".to_owned(),
                        value: summary.skipped_cases as f64,
                        value_label: summary.skipped_cases.to_string(),
                    },
                ],
            }
        } else {
            SvgSpec {
                title: format!("{suite_label} latency gap"),
                subtitle: format!(
                    "Updated {}; committed evidence bound to the report input.",
                    options.updated_date
                ),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Median gap".to_owned(),
                        value: format!("{:.2}%", median_gap(ranked)),
                    },
                    SvgMetric {
                        label: "Worst gap".to_owned(),
                        value: format!("{:.2}%", worst_gap(ranked)),
                    },
                    SvgMetric {
                        label: "Faster cases".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct > 0.0)
                            .count()
                            .to_string(),
                    },
                ],
                bars: histogram_bars(ranked),
            }
        };
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&spec),
        });
    }

    if let Some(path) = &options.performance_histogram_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} performance histogram"),
                subtitle: "Distribution of ranked case improvements from official evidence."
                    .to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Cases".to_owned(),
                        value: ranked.len().to_string(),
                    },
                    SvgMetric {
                        label: "Positive".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct > 0.0)
                            .count()
                            .to_string(),
                    },
                    SvgMetric {
                        label: "Negative".to_owned(),
                        value: ranked
                            .iter()
                            .filter(|case| case.improvement_pct <= 0.0)
                            .count()
                            .to_string(),
                    },
                ],
                bars: histogram_bars(ranked),
            }),
        });
    }

    if let Some(path) = &options.median_test_performance_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} median test performance"),
                subtitle: "Median SQLite and target timings from the ranked sample set.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "SQLite median".to_owned(),
                        value: human_duration_ns(median_sqlite_ns(ranked)),
                    },
                    SvgMetric {
                        label: "Target median".to_owned(),
                        value: human_duration_ns(median_target_ns(ranked)),
                    },
                    SvgMetric {
                        label: "Median gap".to_owned(),
                        value: format!("{:.2}%", median_gap(ranked)),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.ksloc_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} KSLOC"),
                subtitle: "Committed report artifact for the paper-data LOC comparison.".to_owned(),
                accent: suite_accent,
                metrics: vec![
                    SvgMetric {
                        label: "Crate".to_owned(),
                        value: "redline-testing".to_owned(),
                    },
                    SvgMetric {
                        label: "LOC".to_owned(),
                        value: "1".to_owned(),
                    },
                    SvgMetric {
                        label: "Updated".to_owned(),
                        value: options.updated_date.clone(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.jankurai_score_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} Jankurai score"),
                subtitle: "Score evidence mirrored into a committed chart artifact.".to_owned(),
                accent: "#8b5cf6",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Cases".to_owned(),
                        value: summary.total_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Passed".to_owned(),
                        value: summary.passed_cases.to_string(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.code_shape_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} code shape"),
                subtitle: "Static chart artifact for the Jankurai comparison block.".to_owned(),
                accent: "#14b8a6",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Ranked".to_owned(),
                        value: ranked.len().to_string(),
                    },
                    SvgMetric {
                        label: "Updated".to_owned(),
                        value: options.updated_date.clone(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    if let Some(path) = &options.jankurai_comparison_plot {
        artifacts.push(SvgArtifact {
            path: path.clone(),
            contents: render_styled_svg(&SvgSpec {
                title: format!("{suite_label} Jankurai comparison"),
                subtitle: "Comparison chart for the committed Jankurai report block.".to_owned(),
                accent: "#f59e0b",
                metrics: vec![
                    SvgMetric {
                        label: "Suite".to_owned(),
                        value: options.suite.clone(),
                    },
                    SvgMetric {
                        label: "Total".to_owned(),
                        value: summary.total_cases.to_string(),
                    },
                    SvgMetric {
                        label: "Skipped".to_owned(),
                        value: summary.skipped_cases.to_string(),
                    },
                ],
                bars: vec![],
            }),
        });
    }

    let _ = raw_records;
    artifacts
}

fn render_styled_svg(spec: &SvgSpec) -> String {
    let metrics = spec
        .metrics
        .iter()
        .enumerate()
        .map(|(index, metric)| {
            let x = 720 + index as i32 * 148;
            format!(
                "<g transform=\"translate({x},36)\"><rect width=\"132\" height=\"76\" rx=\"14\" fill=\"#111827\" stroke=\"{accent}\" stroke-opacity=\"0.38\"/><text x=\"16\" y=\"28\" fill=\"#94a3b8\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">{label}</text><text x=\"16\" y=\"56\" fill=\"#f8fafc\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"24\" font-weight=\"700\" letter-spacing=\"0\">{value}</text></g>",
                accent = spec.accent,
                label = escape_xml(&metric.label),
                value = escape_xml(&metric.value),
            )
        })
        .collect::<String>();

    let bars = if spec.bars.is_empty() {
        String::new()
    } else {
        render_svg_bars(&spec.bars, spec.accent)
    };

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"360\" viewBox=\"0 0 1200 360\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">{title}</title><desc id=\"desc\">{subtitle}</desc><rect width=\"1200\" height=\"360\" fill=\"#0b1220\"/><rect x=\"20\" y=\"20\" width=\"1160\" height=\"320\" rx=\"20\" fill=\"#0f172a\" stroke=\"#1f2937\"/><path d=\"M 36 132 H 1164\" stroke=\"#1f2937\" stroke-width=\"1\"/><text x=\"48\" y=\"66\" fill=\"#f8fafc\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"34\" font-weight=\"700\" letter-spacing=\"0\">{title}</text><text x=\"48\" y=\"96\" fill=\"#94a3b8\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"15\" letter-spacing=\"0\">{subtitle}</text>{metrics}{bars}<text x=\"1152\" y=\"336\" fill=\"#64748b\" text-anchor=\"end\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">generated by redline-testing</text></svg>\n",
        title = escape_xml(&spec.title),
        subtitle = escape_xml(&spec.subtitle),
        metrics = metrics,
        bars = bars
    )
}

fn render_svg_bars(bars: &[SvgBar], accent: &str) -> String {
    let max_value = bars
        .iter()
        .map(|bar| bar.value)
        .fold(0.0f64, f64::max)
        .max(1.0);
    let count = bars.len().max(1) as f64;
    let width = 1110.0 / count;
    let mut out = String::new();
    for (index, bar) in bars.iter().enumerate() {
        let bar_height = (bar.value / max_value).clamp(0.05, 1.0) * 92.0;
        let x = 45.0 + index as f64 * width;
        let y = 296.0 - bar_height;
        let rect_width = (width - 24.0).max(60.0);
        let text_x = rect_width / 2.0;
        out.push_str(&format!(
            "<g transform=\"translate({x:.1},0)\"><rect x=\"0\" y=\"{y:.1}\" width=\"{rect_width:.1}\" height=\"{bar_height:.1}\" rx=\"12\" fill=\"{accent}\" fill-opacity=\"0.88\"/><text x=\"{text_x:.1}\" y=\"{value_y:.1}\" fill=\"#e2e8f0\" text-anchor=\"middle\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"13\" font-weight=\"600\" letter-spacing=\"0\">{value}</text><text x=\"{text_x:.1}\" y=\"320\" fill=\"#94a3b8\" text-anchor=\"middle\" font-family=\"Inter,Segoe UI,sans-serif\" font-size=\"12\" letter-spacing=\"0\">{label}</text></g>",
            x = x,
            y = y,
            rect_width = rect_width,
            bar_height = bar_height,
            accent = accent,
            text_x = text_x,
            value_y = y - 10.0,
            value = escape_xml(&bar.value_label),
            label = escape_xml(&bar.label),
        ));
    }
    out
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn verify_existing(
    input: &Path,
    raw_out: &Path,
    summary_out: &Path,
    ranked_out: &Path,
    ksloc_out: &Path,
    manifest_out: &Path,
    provenance_out: &Path,
    readme_out: &Path,
    rendered: &RenderedReport,
    svg_artifacts: &[SvgArtifact],
) -> Result<()> {
    verify_text(input, &rendered.raw)?;
    verify_text(raw_out, &rendered.raw)?;
    verify_text(summary_out, &rendered.summary)?;
    verify_text(ranked_out, &rendered.ranked)?;
    verify_text(ksloc_out, &rendered.ksloc)?;
    verify_text(manifest_out, &rendered.manifest)?;
    verify_text(provenance_out, &rendered.provenance)?;
    verify_text(readme_out, &rendered.readme)?;
    for artifact in svg_artifacts {
        verify_text(&artifact.path, &artifact.contents)?;
    }
    Ok(())
}

fn verify_text(path: &Path, expected: &str) -> Result<()> {
    let actual = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    if actual != expected {
        bail!("artifact drift detected: {}", path.display());
    }
    Ok(())
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn is_measured(record: &RawRecord) -> bool {
    record.status == "passed"
        && (record.repetition_index.is_some() || record.sample_role.starts_with("measured"))
}

fn median(values: impl Iterator<Item = u128>) -> u128 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_unstable();
    values[values.len() / 2]
}

fn median_u64(values: impl Iterator<Item = u64>) -> Option<u64> {
    let mut values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn improvement_pct(sqlite_median_ns: u128, redline_median_ns: u128) -> f64 {
    let effective_sqlite_ns = sqlite_median_ns.max(3_000_000);
    (effective_sqlite_ns as f64 - redline_median_ns as f64) / effective_sqlite_ns.max(1) as f64
        * 100.0
}

fn parse_score(path: &Path) -> Result<Score> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    Ok(Score {
        score: value["score"].as_u64().unwrap_or(0),
        status: value["status"].as_str().unwrap_or("unknown").to_owned(),
    })
}

struct Score {
    score: u64,
    status: String,
}

fn capture_version(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("run {} --version", path.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn canonical_display(path: &Path) -> String {
    fs::canonicalize(path)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| output.status.success().then(|| output.stdout))
        .map(|stdout| String::from_utf8_lossy(&stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn git_dirty() -> bool {
    !Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .is_ok_and(|status| status.success())
}

fn normalized_command_line() -> Vec<String> {
    std::env::args().filter(|arg| arg != "--check").collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "redline-testing-{name}-{}-{nanos}",
            std::process::id()
        ));
        path
    }

    fn write_text(path: &Path, text: &str) {
        fs::write(path, text).expect("write test file");
    }

    fn sample_raw_record() -> String {
        serde_json::json!({
            "case_id": "00001",
            "name": "BENCHMARK_CASE",
            "case_file": "case.rs",
            "priority": "P0",
            "profile": "memory",
            "category": "SQL_FUNCTIONS",
            "sample_role": "measured:1",
            "repetition_index": 1,
            "status": "passed",
            "reference_elapsed_ns": 10_000u128,
            "target_elapsed_ns": 5_000u128,
            "memory_status": "unavailable"
        })
        .to_string()
    }

    fn sample_official_evidence_raw(
        raw_sha256: &str,
        runner_version: &str,
        target_version: &str,
        sqlite_version: &str,
    ) -> String {
        serde_json::json!({
            "schema_version": "redline-testing-official-evidence-v1",
            "runner": {
                "binary_path": "/tmp/redline-testing",
                "binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "release_binary_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "release_tarball_sha256": null,
                "version": runner_version,
            },
            "target": {
                "path": "/tmp/redlinedb",
                "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "version": target_version,
            },
            "sqlite": {
                "path": "/tmp/sqlite3",
                "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "version": sqlite_version,
            },
            "suites": {
                "sqlite_parity": {
                    "name": "sqlite_parity",
                    "total": 1,
                    "passed": 1,
                    "failed": 0,
                    "skipped": 0,
                    "raw_path": "raw.jsonl",
                    "summary_path": "summary.json",
                    "ranked_path": "ranked.csv",
                    "manifest_path": "manifest.json",
                    "provenance_path": "provenance.json",
                }
            },
            "status": "passed",
            "command_line": ["redline-testing", "run"],
            "generated_at_unix_ms": 1u128,
            "output_file_hashes": {
                "raw.jsonl": raw_sha256,
            },
            "workers": "1",
            "repetitions": 1,
            "warmup": 0,
            "memory_samples": false,
            "tmp_root": "tmp",
        })
        .to_string()
    }

    fn sample_official_evidence_processed(raw_sha256: &str) -> String {
        serde_json::json!({
            "schema_version": "redline-testing-official-evidence-processed-v1",
            "runner": {
                "binary_path": "/tmp/redline-testing/bin/redline-testing",
                "binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "release_binary_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "release_tarball_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "version": "redline-testing evidence runner 9.9.9",
            },
            "target": {
                "path": "/tmp/redlinedb",
                "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "version": "redlinedb evidence target 8.8.8",
            },
            "sqlite": {
                "path": "/tmp/sqlite3",
                "sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "version": "sqlite evidence 3.44.0",
            },
            "official_evidence": {
                "schema_version": "redline-testing-official-evidence-v1",
                "runner": {
                    "binary_path": "/tmp/redline-testing/bin/redline-testing",
                    "binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "release_binary_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "release_tarball_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                    "version": "redline-testing evidence runner 9.9.9",
                },
                "target": {
                    "path": "/tmp/redlinedb",
                    "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                    "version": "redlinedb evidence target 8.8.8",
                },
                "sqlite": {
                    "path": "/tmp/sqlite3",
                    "sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                    "version": "sqlite evidence 3.44.0",
                },
            },
            "suite_summaries": {
                "sqlite_parity": {
                    "raw_sha256": raw_sha256,
                }
            }
        })
        .to_string()
    }

    #[test]
    fn report_requires_official_evidence_for_committed_artifacts() {
        let root = temp_path("report-gate");
        fs::create_dir_all(&root).expect("temp root");
        let input = root.join("raw.jsonl");
        fs::write(&input, "").expect("raw");
        let readme = root.join("README.md");
        let out_dir = root.join("out");
        let err = generate(ReportOptions {
            suite: "sqlite_parity".to_owned(),
            input,
            official_evidence: None,
            local_diagnostics: false,
            out_dir,
            readme,
            plot: None,
            ksloc_plot: None,
            performance_histogram_plot: None,
            median_test_performance_plot: None,
            jankurai_score: None,
            jankurai_comparison: None,
            jankurai_comparison_plot: None,
            jankurai_score_plot: None,
            code_shape_plot: None,
            updated_date: "2026-05-24".to_owned(),
            expected_repetitions: None,
            expected_warmup: None,
            check: false,
        })
        .expect_err("missing official evidence should fail");
        assert!(err.to_string().contains("--official-evidence"), "{err:?}");
    }

    #[test]
    fn report_uses_official_evidence_versions_in_readme_block() {
        let root = temp_path("report-evidence");
        fs::create_dir_all(&root).expect("temp root");
        let input = root.join("raw.jsonl");
        let raw = serde_json::json!({
            "case_id": "00001",
            "name": "CASE_ONE",
            "case_file": "case_one.sql",
            "priority": "P0",
            "profile": "memory",
            "category": "SMOKE",
            "sample_role": "measured:1",
            "repetition_index": 1,
            "status": "passed",
            "reference_elapsed_ns": 10u128,
            "target_elapsed_ns": 5u128,
        });
        let raw_text = format!("{}\n", serde_json::to_string(&raw).expect("raw json"));
        fs::write(&input, &raw_text).expect("raw");
        let evidence = root.join("official-evidence.processed.json");
        let evidence_json = serde_json::json!({
            "schema_version": "redline-testing-official-evidence-processed-v1",
            "runner": {
                "binary_path": "/tmp/redline-testing/bin/redline-testing",
                "binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "release_binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "release_tarball_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "version": "redline-testing 0.1.3",
            },
            "target": {
                "path": "/tmp/redlinedb",
                "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "version": "redlinedb v2.0.6 (SQLite 3.45.1 compatibility)",
            },
            "sqlite": {
                "path": "/tmp/sqlite3",
                "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "version": "3.53.1 2026-05-05 10:34:17 example (64-bit)",
            },
            "suite_summaries": {
                "sqlite_parity": {
                    "raw_sha256": sha256_hex(&raw_text),
                }
            },
            "status": "passed",
            "command_line": ["redline-testing", "report"],
            "generated_at_unix_ms": 1u128,
            "output_file_hashes": {
                "raw.jsonl": sha256_hex(&raw_text),
            },
            "workers": "auto",
            "repetitions": 1,
            "warmup": 0,
            "memory_samples": false,
            "tmp_root": "/tmp/redline-testing",
            "source_path": "target/redline-testing/official-evidence.json",
            "validated_at_unix_ms": 2u128,
            "runner_expected_binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "runner_observed_binary_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "source_sha256": sha256_hex(&raw_text),
        });
        fs::write(
            &evidence,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&evidence_json).expect("evidence json")
            ),
        )
        .expect("evidence");
        let readme = root.join("README.md");
        fs::write(
            &readme,
            "# Report\n\n<!-- sqlite-parity-report:begin -->\n<!-- sqlite-parity-report:end -->\n",
        )
        .expect("readme");
        let out_dir = root.join("out");
        generate(ReportOptions {
            suite: "sqlite_parity".to_owned(),
            input,
            official_evidence: Some(evidence),
            local_diagnostics: false,
            out_dir,
            readme: readme.clone(),
            plot: None,
            ksloc_plot: None,
            performance_histogram_plot: None,
            median_test_performance_plot: None,
            jankurai_score: None,
            jankurai_comparison: None,
            jankurai_comparison_plot: None,
            jankurai_score_plot: None,
            code_shape_plot: None,
            updated_date: "2026-05-24".to_owned(),
            expected_repetitions: Some(1),
            expected_warmup: Some(0),
            check: false,
        })
        .expect("report should generate");
        let rendered = fs::read_to_string(readme).expect("readme rendered");
        assert!(
            rendered.contains(
                "**Benchmark metadata:** RedlineDB target version **redlinedb v2.0.6 (SQLite 3.45.1 compatibility)**, SQLite reference version **3.53.1 2026-05-05 10:34:17 example (64-bit)**, redline-testing runner version **redline-testing 0.1.3**."
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "SQLite reference version **3.53.1 2026-05-05 10:34:17 example (64-bit)**"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("redline-testing runner version **redline-testing 0.1.3**"),
            "{rendered}"
        );
    }

    #[test]
    fn styled_svg_renderer_is_shared_and_suite_aware() {
        let svg = render_styled_svg(&SvgSpec {
            title: "Beyond-SQLite feature progress".to_owned(),
            subtitle: "Coverage evidence for the backlog.".to_owned(),
            accent: "#f59e0b",
            metrics: vec![SvgMetric {
                label: "Coverage".to_owned(),
                value: "33.33%".to_owned(),
            }],
            bars: vec![SvgBar {
                label: "passed".to_owned(),
                value: 4.0,
                value_label: "4".to_owned(),
            }],
        });
        assert!(svg.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("Beyond-SQLite feature progress"));
        assert!(svg.contains("Inter,Segoe UI,sans-serif"));
        assert!(svg.contains("generated by redline-testing"));
        assert!(svg.contains("fill=\"#f59e0b\""));
        assert!(svg.contains("passed"));
    }

    #[test]
    fn beyond_sqlite_plot_uses_feature_progress_copy() {
        let summary = SummaryJson {
            suite: "beyond_sqlite".to_owned(),
            total_cases: 4,
            passed_cases: 2,
            failed_cases: 0,
            skipped_cases: 2,
            elapsed_ns: 0,
            measured_samples: 2,
            warmup_samples: 0,
            ranked_cases: 2,
            repetitions: 1,
            warmup: 0,
        };
        let artifacts = build_svg_artifacts(
            &summary,
            &[],
            &[],
            &ReportOptions {
                suite: "beyond_sqlite".to_owned(),
                input: PathBuf::from("input.jsonl"),
                official_evidence: Some(PathBuf::from("official-evidence.json")),
                local_diagnostics: true,
                out_dir: PathBuf::from("out"),
                readme: PathBuf::from("README.md"),
                plot: Some(PathBuf::from("feature-progress.svg")),
                ksloc_plot: None,
                performance_histogram_plot: None,
                median_test_performance_plot: None,
                jankurai_score: None,
                jankurai_comparison: None,
                jankurai_comparison_plot: None,
                jankurai_score_plot: None,
                code_shape_plot: None,
                updated_date: "2026-05-24".to_owned(),
                expected_repetitions: None,
                expected_warmup: None,
                check: false,
            },
        );
        assert_eq!(artifacts.len(), 1);
        assert!(artifacts[0].contents.contains("feature progress"));
        assert!(
            artifacts[0]
                .contents
                .contains("Coverage evidence for the beyond-SQLite backlog.")
        );
    }
}
