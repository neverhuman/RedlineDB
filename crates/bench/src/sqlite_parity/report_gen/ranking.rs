use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::io::csv;
use super::model::{RankedCase, SummaryJson};
use super::{
    LATENCY_REFERENCE_FLOOR_NS, MIN_FASTER_CASES, MIN_MEDIAN_IMPROVEMENT_PCT,
    MIN_WORST_IMPROVEMENT_PCT,
};

#[derive(Debug, Deserialize)]
pub(super) struct RawRecord {
    pub(super) case_id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) case_file: String,
    pub(super) priority: String,
    pub(super) profile: String,
    pub(super) category: String,
    #[serde(default)]
    pub(super) sample_role: String,
    #[serde(default)]
    pub(super) repetition_index: Option<usize>,
    #[serde(default)]
    pub(super) sqlite_version: Option<String>,
    #[serde(default)]
    pub(super) reference_engine: String,
    #[serde(default)]
    pub(super) target_engine: String,
    #[serde(default)]
    pub(super) reference_executable_path: String,
    #[serde(default)]
    pub(super) target_executable_path: String,
    #[serde(default)]
    pub(super) reference_executable_sha256: String,
    #[serde(default)]
    pub(super) target_executable_sha256: String,
    #[serde(default)]
    pub(super) reference_version: String,
    #[serde(default)]
    pub(super) target_version: String,
    pub(super) status: String,
    pub(super) reference_elapsed_ns: u128,
    pub(super) target_elapsed_ns: u128,
}

#[derive(Debug)]
pub(super) struct BuiltReport {
    pub(super) ranked: Vec<RankedCase>,
    pub(super) summary: SummaryJson,
    pub(super) repetitions: usize,
    pub(super) warmup: usize,
    pub(super) coverage_failures: Vec<String>,
    pub(super) performance_failures: Vec<String>,
}

pub(super) fn parse_raw_records(raw_text: &str) -> Result<Vec<RawRecord>> {
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

pub(super) fn build_report(
    all_cases: &[super::super::case::Case],
    expected: &BTreeSet<String>,
    raw_records: Vec<RawRecord>,
    updated_date: &str,
    git_sha: &str,
    expected_repetitions: Option<usize>,
    expected_warmup: Option<usize>,
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
        let warmups_for_case = records.iter().filter(|record| is_warmup(record)).count();
        if let Some(expected_warmup) = expected_warmup
            && warmups_for_case != expected_warmup
        {
            missing_cases = missing_cases.saturating_add(1);
            coverage_failures.push(format!(
                "{id} warmup samples {warmups_for_case} != expected {expected_warmup}"
            ));
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
        if let Some(expected_repetitions) = expected_repetitions
            && passed.len() != expected_repetitions
        {
            missing_cases = missing_cases.saturating_add(1);
            coverage_failures.push(format!(
                "{id} measured samples {} != expected {expected_repetitions}",
                passed.len()
            ));
            continue;
        }
        if passed
            .iter()
            .any(|record| !has_execution_provenance(record))
        {
            missing_cases = missing_cases.saturating_add(1);
            coverage_failures.push(format!("{id} lacks target execution provenance"));
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
    let sqlite_case_median_ns = median_ranked_value(&ranked, |case| case.sqlite_median_ns);
    let redline_case_median_ns = median_ranked_value(&ranked, |case| case.redline_median_ns);
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
            sqlite_case_median_ns,
            redline_case_median_ns,
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

pub(super) fn ranked_csv(ranked: &[RankedCase]) -> String {
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

fn is_warmup(record: &RawRecord) -> bool {
    record.sample_role == "warmup"
}

fn is_measured(record: &RawRecord) -> bool {
    record.status == "passed"
        && (record.repetition_index.is_some()
            || record.sample_role.starts_with("measured")
            || record.sample_role.is_empty())
}

fn has_execution_provenance(record: &RawRecord) -> bool {
    (record.reference_engine.eq_ignore_ascii_case("sqlite3")
        || record.reference_engine.eq_ignore_ascii_case("sqlite"))
        && record.target_engine.eq_ignore_ascii_case("redlinedb")
        && !record.reference_executable_path.is_empty()
        && !record.target_executable_path.is_empty()
        && record.reference_executable_path != record.target_executable_path
        && !record.reference_executable_sha256.is_empty()
        && !record.target_executable_sha256.is_empty()
        && record.reference_executable_sha256 != record.target_executable_sha256
        && !record.reference_version.is_empty()
        && record
            .target_version
            .to_ascii_lowercase()
            .contains("redlinedb")
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

fn median_ranked_value(ranked: &[RankedCase], value: impl Fn(&RankedCase) -> u128) -> u128 {
    if ranked.is_empty() {
        return 0;
    }
    let mut values = ranked.iter().map(value).collect::<Vec<_>>();
    values.sort_unstable();
    values[values.len() / 2]
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
