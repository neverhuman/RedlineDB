use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone)]
pub(crate) struct RankedCase {
    pub(super) case_id: String,
    pub(super) name: String,
    pub(super) case_file: String,
    pub(super) priority: String,
    pub(super) profile: String,
    pub(super) category: String,
    pub(super) sqlite_median_ns: u128,
    pub(super) redline_median_ns: u128,
    pub(super) improvement_pct: f64,
    pub(super) samples: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct SummaryJson {
    pub(super) updated_date: String,
    pub(super) git_sha: String,
    pub(super) sqlite_version: String,
    pub(super) generated_cases: usize,
    pub(super) expected_cases: usize,
    pub(super) passed_cases: usize,
    pub(super) failed_cases: usize,
    pub(super) missing_cases: usize,
    pub(super) skipped_cases: usize,
    pub(super) ranked_cases: usize,
    pub(super) coverage_pct: f64,
    pub(super) measured_samples: usize,
    pub(super) warmup_samples: usize,
    pub(super) sqlite_case_median_ns: u128,
    pub(super) redline_case_median_ns: u128,
    pub(super) median_latency_gap_pct: f64,
    pub(super) worst_latency_gap_pct: f64,
    pub(super) faster_cases: usize,
    pub(super) latency_reference_floor_ns: u128,
}

#[derive(Debug, Serialize)]
pub(crate) struct ManifestJson {
    pub(super) command: Vec<String>,
    pub(super) git_sha: String,
    pub(super) sqlite_version: String,
    pub(super) updated_date: String,
    pub(super) repetitions: usize,
    pub(super) warmup: usize,
    pub(super) input_hashes: BTreeMap<String, String>,
    pub(super) output_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JankuraiScore {
    pub(super) score: u64,
    pub(super) status: String,
    pub(super) color: &'static str,
}
