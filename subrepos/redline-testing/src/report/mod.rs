mod compare;
mod evidence;
mod render;
mod svg;
mod types;
mod utils;

#[cfg(test)]
mod tests;

pub use compare::{jankurai_compare, sentinel};
pub use types::{JankuraiCompareOptions, ReportOptions, SentinelOptions};

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use evidence::{
    artifact_names_for_suite, read_official_evidence_versions, validate_official_evidence_binding,
};
use render::{
    ksloc_csv, parse_raw_records, rank_cases, ranked_csv, render_metrics_block,
    render_report_block, replace_block,
};
use svg::build_svg_artifacts;
use types::{ManifestJson, ProvenanceJson, RawRecord, RenderedReport, SummaryJson};
use utils::{
    canonical_display, git_dirty, git_sha, normalized_command_line, sha256_file, sha256_hex,
    verify_existing, write_text,
};

const REPORT_BEGIN: &str = "<!-- sqlite-parity-report:begin -->";
const REPORT_END: &str = "<!-- sqlite-parity-report:end -->";
const METRICS_BEGIN: &str = "<!-- sqlite-parity-metrics:begin -->";
const METRICS_END: &str = "<!-- sqlite-parity-metrics:end -->";
const JANKURAI_BREAKDOWN_BEGIN: &str = "<!-- sqlite-jankurai-breakdown:begin -->";
const JANKURAI_BREAKDOWN_END: &str = "<!-- sqlite-jankurai-breakdown:end -->";

fn validate_warmups(records: &[RawRecord], expected_warmup: usize) -> Result<()> {
    let mut cases = BTreeMap::<&str, (bool, usize)>::new();
    for record in records {
        let (executed, warmups) = cases.entry(&record.case_id).or_default();
        *executed |= record.status != "skipped";
        *warmups += usize::from(record.sample_role == "warmup");
    }
    for (case_id, (executed, warmups)) in cases {
        // A declared skip is one placeholder record, with no benchmark samples.
        let expected = if executed { expected_warmup } else { 0 };
        if warmups != expected {
            bail!("case {case_id}: expected {expected} warmup samples but found {warmups}");
        }
    }
    Ok(())
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
            .filter(|record| utils::is_measured(record))
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
        validate_warmups(&raw_records, expected_warmup)?;
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
        .filter(|record| record.status == "passed" && utils::is_measured(record))
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
        .filter(|record| utils::is_measured(record))
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
    let target_version =
        utils::capture_version(&target_bin).unwrap_or_else(|_| "<unknown>".to_owned());
    let sqlite_binary_path = std::env::var_os("REDLINE_TESTING_SQLITE_BIN")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(crate::sqlite_parity::REFERENCE_CLI_BIN));
    let sqlite_binary_sha256 =
        sha256_file(&sqlite_binary_path).unwrap_or_else(|_| "<unknown>".to_owned());
    let sqlite_version =
        utils::capture_version(&sqlite_binary_path).unwrap_or_else(|_| "<unknown>".to_owned());
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
