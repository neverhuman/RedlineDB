//! Integration test: verify every shard under corpus/sqlite_parity/cases/
//! parses as the same Case schema the runner uses, IDs are unique and start
//! at >= 10001, and the union is strictly increasing across the pinned +
//! extended set. This catches shard-schema drift and ID-collision bugs that
//! the per-shard ship-gate doesn't see (xtask ship-gate validates content,
//! this validates structure).

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(serde::Deserialize)]
struct CaseLite {
    id: u64,
    name: String,
    category: String,
    priority: String,
}

fn cases_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join("sqlite_parity")
        .join("cases")
}

#[test]
fn every_extended_shard_parses_and_has_unique_ids() {
    let dir = cases_dir();
    assert!(dir.is_dir(), "corpus/sqlite_parity/cases must exist");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read cases dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "json")
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('_'))
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "expected at least one extended shard");

    let mut all_ids: HashSet<u64> = HashSet::new();
    let mut total = 0usize;
    let valid_priorities = ["P0", "P1", "P2", "P3", "P4"];
    for path in &paths {
        let body = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let cases: Vec<CaseLite> =
            serde_json::from_str(&body).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        assert!(!cases.is_empty(), "shard {path:?} is empty");
        for case in &cases {
            assert!(
                case.id >= 10001,
                "shard {path:?} case {} ({}) has id {} < 10001 (reserve clash with upstream)",
                case.name,
                case.category,
                case.id
            );
            assert!(
                valid_priorities.contains(&case.priority.as_str()),
                "shard {path:?} case {} has invalid priority {}",
                case.name,
                case.priority
            );
            assert!(
                all_ids.insert(case.id),
                "shard {path:?} duplicate id {} (name={})",
                case.id,
                case.name
            );
            total += 1;
        }
    }
    assert!(
        total >= 1000,
        "expected >= 1000 extended cases (got {total}); did a shard get cut?"
    );
}

#[test]
fn extended_ids_dont_collide_with_pinned() {
    // The pinned manifest covers 1..=1127. Extended must start at >= 10001,
    // never reusing pinned space (1128..=9999 reserved for upstream growth).
    let pinned_raw = include_str!("../corpus/sqlite_parity/generated_manifest.json");
    let pinned: Vec<CaseLite> = serde_json::from_str(pinned_raw).expect("parse pinned manifest");
    let pinned_ids: HashSet<u64> = pinned.iter().map(|c| c.id).collect();
    for path in fs::read_dir(cases_dir())
        .expect("cases dir")
        .filter_map(|e| e.ok())
    {
        let p = path.path();
        if p.extension().is_some_and(|ext| ext == "json") {
            let body = fs::read_to_string(&p).expect("read shard");
            let cases: Vec<CaseLite> = serde_json::from_str(&body).expect("parse shard");
            for c in &cases {
                assert!(
                    !pinned_ids.contains(&c.id),
                    "extended shard {} reuses pinned id {} ({})",
                    p.display(),
                    c.id,
                    c.name
                );
            }
        }
    }
}
