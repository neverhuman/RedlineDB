use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::types::{ArtifactNames, EvidenceVersions, RankedCase, RawRecord, SvgBar};
use super::utils::median_u64;

pub(crate) fn validate_official_evidence_binding(
    official_evidence: &Path,
    suite: &str,
    raw_text: &str,
) -> Result<()> {
    let text = fs::read_to_string(official_evidence)
        .with_context(|| format!("read official evidence {}", official_evidence.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("parse official evidence {}", official_evidence.display()))?;
    let actual_raw_sha256 = super::utils::sha256_hex(raw_text);
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

pub(crate) fn read_official_evidence_versions(
    official_evidence: &Path,
) -> Result<EvidenceVersions> {
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

pub(crate) fn suite_display_name(suite: &str) -> String {
    match suite {
        "memory" => "Memory".to_owned(),
        "beyond_sqlite" => "Beyond-SQLite".to_owned(),
        "sqlite_parity" => "SQLite parity".to_owned(),
        "rql_phase1" => "RQL phase 1".to_owned(),
        "all" => "All suites".to_owned(),
        other => other.replace('_', " "),
    }
}

pub(crate) fn suite_subject(suite: &str) -> &'static str {
    match suite {
        "beyond_sqlite" => "features",
        _ => "cases",
    }
}

pub(crate) fn suite_accent(suite: &str) -> &'static str {
    match suite {
        "memory" => "#14b8a6",
        "beyond_sqlite" => "#f59e0b",
        "sqlite_parity" => "#38bdf8",
        "rql_phase1" => "#a855f7",
        _ => "#64748b",
    }
}

pub(crate) fn memory_status_summary(records: &[RawRecord]) -> &'static str {
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

pub(crate) fn memory_peak_summary(records: &[RawRecord]) -> Option<String> {
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

pub(crate) fn median_gap(ranked: &[RankedCase]) -> f64 {
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

pub(crate) fn worst_gap(ranked: &[RankedCase]) -> f64 {
    ranked
        .iter()
        .map(|case| case.improvement_pct)
        .min_by(|left, right| left.total_cmp(right))
        .unwrap_or(0.0)
}

pub(crate) fn median_sqlite_ns(ranked: &[RankedCase]) -> u128 {
    if ranked.is_empty() {
        return 0;
    }
    super::utils::median(ranked.iter().map(|case| case.sqlite_median_ns))
}

pub(crate) fn median_target_ns(ranked: &[RankedCase]) -> u128 {
    if ranked.is_empty() {
        return 0;
    }
    super::utils::median(ranked.iter().map(|case| case.redline_median_ns))
}

pub(crate) fn human_duration_ns(value: u128) -> String {
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

pub(crate) fn histogram_bars(ranked: &[RankedCase]) -> Vec<SvgBar> {
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

pub(crate) fn artifact_names_for_suite(suite: &str) -> ArtifactNames {
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
