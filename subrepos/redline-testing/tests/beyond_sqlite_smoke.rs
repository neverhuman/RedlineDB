//! Integration test: verify the beyond-SQLite feature manifest and the
//! executable case manifest both parse. NO Postgres dependency — this is the
//! fast lane that runs on any machine.

use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct FeatureLite {
    rank: usize,
    title: String,
    owner: String,
    proof_lane: String,
    status: String,
}

#[derive(serde::Deserialize)]
struct CaseLite {
    id: u64,
    name: String,
    feature_rank: usize,
    category: String,
    priority: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn features_json_parses_and_ranks_are_strict_increasing() {
    let raw = std::fs::read_to_string(repo_root().join("metadata/beyond_sqlite/features.json"))
        .expect("read features.json");
    let features: Vec<FeatureLite> = serde_json::from_str(&raw).expect("parse features.json");
    assert!(features.len() >= 10, "expected >= 10 ranked features");
    assert!(
        features.windows(2).all(|pair| pair[0].rank < pair[1].rank),
        "ranks must be strictly increasing"
    );
    for f in &features {
        assert!(!f.title.is_empty(), "feature {} has empty title", f.rank);
        assert!(!f.owner.is_empty(), "feature {} has empty owner", f.rank);
        assert!(
            !f.proof_lane.is_empty(),
            "feature {} has empty proof_lane",
            f.rank
        );
        assert!(
            matches!(f.status.as_str(), "manifest_backlog" | "passing_reference"),
            "feature {} has unexpected status {}",
            f.rank,
            f.status
        );
    }
}

#[test]
fn beyond_case_manifest_parses_and_is_ordered() {
    let raw =
        std::fs::read_to_string(repo_root().join("corpus/beyond_sqlite/generated_manifest.json"))
            .expect("read beyond case manifest");
    let cases: Vec<CaseLite> =
        serde_json::from_str(&raw).expect("parse corpus/beyond_sqlite/generated_manifest.json");
    let valid_priorities = ["P0", "P1", "P2", "P3", "P4"];
    let mut prev_id: Option<u64> = None;
    for case in &cases {
        assert!(
            case.id >= 20001,
            "beyond case {} has id {} < 20001 (reserve)",
            case.name,
            case.id
        );
        assert!(
            valid_priorities.contains(&case.priority.as_str()),
            "beyond case {} has invalid priority {}",
            case.name,
            case.priority
        );
        assert!(
            (1..=99).contains(&case.feature_rank),
            "beyond case {} has implausible feature_rank {}",
            case.name,
            case.feature_rank
        );
        assert!(
            !case.category.is_empty(),
            "beyond case {} has empty category",
            case.name
        );
        if let Some(prev) = prev_id {
            assert!(
                prev < case.id,
                "beyond case ids must be strictly increasing: {prev} >= {}",
                case.id
            );
        }
        prev_id = Some(case.id);
    }
}
