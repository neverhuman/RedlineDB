use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::generate;
use super::svg::{build_svg_artifacts, render_styled_svg};
use super::types::{ReportOptions, SummaryJson, SvgBar, SvgMetric, SvgSpec};
use super::utils::sha256_hex;

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

fn warmup_fixture(case_id: &str, role: &str, status: &str) -> super::types::RawRecord {
    let mut value: serde_json::Value = serde_json::from_str(&sample_raw_record()).unwrap();
    value["case_id"] = case_id.into();
    value["sample_role"] = role.into();
    value["status"] = status.into();
    serde_json::from_value(value).unwrap()
}

#[test]
fn warmup_validation_accepts_declared_skips_without_samples() {
    let records = [
        warmup_fixture("executed", "warmup", "passed"),
        warmup_fixture("executed", "measured:1", "passed"),
        warmup_fixture("skipped", "skipped", "skipped"),
    ];
    super::validate_warmups(&records, 1).unwrap();
}

#[test]
fn warmup_validation_rejects_missing_samples_even_with_declared_skips() {
    let records = [
        warmup_fixture("executed", "measured:1", "passed"),
        warmup_fixture("skipped", "skipped", "skipped"),
    ];
    let err = super::validate_warmups(&records, 1).unwrap_err();
    assert!(
        err.to_string()
            .contains("case executed: expected 1 warmup samples but found 0")
    );
}

#[test]
fn warmup_validation_rejects_balanced_missing_and_duplicate_samples() {
    let records = [
        warmup_fixture("missing", "measured:1", "passed"),
        warmup_fixture("extra", "warmup", "passed"),
        warmup_fixture("extra", "warmup", "passed"),
        warmup_fixture("extra", "measured:1", "passed"),
    ];
    assert!(super::validate_warmups(&records, 1).is_err());
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
            "path": "/tmp/sqlite-cli",
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
            "path": "/tmp/sqlite-cli",
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
                "path": "/tmp/sqlite-cli",
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
            "path": "/tmp/sqlite-cli",
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
        rendered
            .contains("SQLite reference version **3.53.1 2026-05-05 10:34:17 example (64-bit)**"),
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
