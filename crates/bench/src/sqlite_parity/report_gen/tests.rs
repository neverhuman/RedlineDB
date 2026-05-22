use std::fs;
use std::path::Path;

use super::io::{existing_manifest_git_sha, normalize_git_sha, resolve_git_sha};
use super::ranking::{RawRecord, build_report};
use super::readme::{
    jankurai_badge_block, metrics_block, parse_jankurai_score, readme_block,
    replace_jankurai_badge, replace_readme_block,
};
use super::svg::{beyond_sqlite_feature_progress_svg, ksloc_svg, latency_svg};
use super::{
    JANKURAI_BADGE_BEGIN, JANKURAI_BADGE_END, JankuraiScore, LATENCY_REFERENCE_FLOOR_NS,
    README_BEGIN, README_END, catalog, performance_histogram, source_lines,
};
use super::{RankedCase, SummaryJson};

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
        sqlite_case_median_ns: 100,
        redline_case_median_ns: 90,
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
            improvement_pct: -100.0,
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
            improvement_pct: 50.0,
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
    let expected = std::collections::BTreeSet::from(["00001".to_owned()]);
    let report = build_report(&all_cases, &expected, records, "2026-05-20", "sha").expect("report");
    assert_eq!(report.ranked[0].sqlite_median_ns, 300);
    assert_eq!(report.ranked[0].redline_median_ns, 120);
    assert_eq!(report.summary.sqlite_case_median_ns, 300);
    assert_eq!(report.summary.redline_case_median_ns, 120);
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
    let expected = std::collections::BTreeSet::from(["00001".to_owned()]);
    let report = build_report(&all_cases, &expected, records, "2026-05-20", "sha").expect("report");
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
    let expected = std::collections::BTreeSet::from([
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
    let expected = std::collections::BTreeSet::from(["99999".to_owned()]);

    let err =
        build_report(&all_cases, &expected, records, "2026-05-20", "sha").expect_err("metadata");

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
        Path::new("assets/beyond-sqlite-feature-progress.svg"),
        Path::new("assets/sqlite-parity-ksloc.svg"),
        Some(Path::new("assets/sqlite-jankurai-score.svg")),
        Some(Path::new("assets/sqlite-code-shape.svg")),
        Some(Path::new("assets/sqlite-median-test-performance.svg")),
    );
    assert!(block.contains("[Full ranked latency table](#sqlite-parity-ranked-latency-table)"));
    assert!(metrics.contains(
        "![Beyond-SQLite feature progress chart](assets/beyond-sqlite-feature-progress.svg)"
    ));
    assert!(
        metrics.contains(
            "![SQLite vs RedlineDB production KSLOC chart](assets/sqlite-parity-ksloc.svg)"
        )
    );
    assert!(
        metrics.contains(
            "![RedlineDB vs SQLite Jankurai score chart](assets/sqlite-jankurai-score.svg)"
        )
    );
    assert!(
        metrics.contains(
            "![RedlineDB vs SQLite code shape score chart](assets/sqlite-code-shape.svg)"
        )
    );
    assert!(metrics.contains(
        "![RedlineDB vs SQLite median test performance chart](assets/sqlite-median-test-performance.svg)"
    ));
    assert!(block.contains("<details id=\"sqlite-parity-ranked-latency-table\">"));
}

#[test]
fn beyond_sqlite_feature_progress_chart_counts_reference_rows() {
    let backlog = include_str!("../../../../../docs/beyond-sqlite-gaps.md");
    let svg = beyond_sqlite_feature_progress_svg(backlog, "2026-05-20").expect("svg");
    assert!(svg.contains("4 / 12"));
    assert!(svg.contains("Beyond-SQLite feature progress"));
    assert!(svg.contains("Passing reference"));
}

#[test]
fn jankurai_badge_renders_score_status_and_color() {
    let score = parse_jankurai_score(r#"{ "score": 64, "decision": { "status": "advisory" } }"#)
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
    let current = "<p align=\"center\">\n  <img src=\"assets/redlinedb-banner.png\" alt=\"RedlineDB\" width=\"100%\">\n</p>\n\n<p align=\"center\">\n  <a href=\"LICENSE\"><img src=\"license.svg\" alt=\"license\"></a>\n  <img src=\"https://img.shields.io/badge/version-2.0.0-blue\" alt=\"version\">\n</p>\nafter\n";
    let next = replace_jankurai_badge(current, &score).expect("replace");

    assert!(next.contains("<a href=\"LICENSE\"><img src=\"license.svg\" alt=\"license\"></a>"));
    assert!(
        next.contains(
            "<img src=\"https://img.shields.io/badge/version-2.0.0-blue\" alt=\"version\">"
        )
    );
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
