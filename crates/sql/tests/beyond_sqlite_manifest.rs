use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const BACKLOG: &str = include_str!("../../../docs/beyond-sqlite-gaps.md");
const EXPECTED_GAPS: &[&str] = &[
    "Multi-writer / row-locking / queue semantics: `FOR UPDATE`, `SKIP LOCKED`, concurrent row reservations",
    "Migration ergonomics: `ALTER COLUMN`, defaults, constraint add/drop, safer table evolution",
    "Stored SQL routines: SQL functions/procedures, variables, reusable DB-side logic",
    "Replication / sync / CDC: manifest first; executable tests wait for a RedlineDB API",
    "`LISTEN` / `NOTIFY`: Postgres reference, RedlineDB event contract later",
    "Materialized views: create, refresh, indexed refresh targets",
    "Richer typing: decimal, UUID, boolean, timestamps, stricter mode",
    "Unicode/collation/`ILIKE`: start with active Postgres-vs-RedlineDB `ILIKE` tests because RedlineDB already has partial support",
    "JSONB/document indexing: containment, path lookup, indexed generated path cases",
    "Schemas/sequences/identity: namespaces, sequence objects, identity syntax",
    "SQL portability syntax: `MERGE`, `LATERAL`, data-modifying CTEs, `DISTINCT ON`, `DEFAULT` in values",
    "Advanced indexes/search/vector: manifest entries first unless existing implementations already pass",
];

#[derive(Debug)]
struct Gap {
    rank: usize,
    title: String,
    owner: String,
    proof_lane: String,
    sources: Vec<String>,
}

#[test]
fn beyond_sqlite_backlog_is_ranked_and_stable() {
    let gaps = parse_backlog();
    assert_eq!(gaps.len(), EXPECTED_GAPS.len(), "unexpected backlog length");
    for (index, gap) in gaps.iter().enumerate() {
        assert_eq!(gap.rank, index + 1, "rank sequence drifted");
        assert_eq!(gap.title, EXPECTED_GAPS[index], "gap title drifted");
    }
}

#[test]
fn beyond_sqlite_backlog_sources_all_committed_tips() {
    let repo = repo_root();
    let source_files = parse_backlog()
        .into_iter()
        .flat_map(|gap| gap.sources)
        .collect::<BTreeSet<_>>();
    let committed_tips = fs::read_dir(repo.join("tips/beyond"))
        .expect("read tips/beyond")
        .map(|entry| {
            entry
                .expect("tip entry")
                .file_name()
                .into_string()
                .expect("utf8 tip filename")
        })
        .filter(|name| name.ends_with(".txt"))
        .collect::<BTreeSet<_>>();

    assert_eq!(
        source_files, committed_tips,
        "docs/beyond-sqlite-gaps.md must cite every tips/beyond/*.txt source"
    );
}

#[test]
fn beyond_sqlite_backlog_maps_to_known_owners_and_lanes() {
    let repo = repo_root();
    let owners = owner_names(&repo);
    let lanes = proof_lane_names(&repo);

    for gap in parse_backlog() {
        assert!(
            owners.contains(gap.owner.as_str()),
            "unknown owner `{}` for rank {}",
            gap.owner,
            gap.rank
        );
        assert!(
            lanes.contains(gap.proof_lane.as_str()),
            "unknown proof lane `{}` for rank {}",
            gap.proof_lane,
            gap.rank
        );
    }
}

fn parse_backlog() -> Vec<Gap> {
    BACKLOG
        .lines()
        .filter_map(parse_table_row)
        .collect::<Vec<_>>()
}

fn parse_table_row(line: &str) -> Option<Gap> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let cells = trimmed
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();
    if cells.len() < 6 {
        return None;
    }
    let rank = cells[0].parse::<usize>().ok()?;
    let sources = cells[4]
        .split(',')
        .map(|source| source.trim().trim_matches('`').to_owned())
        .filter(|source| !source.is_empty())
        .collect::<Vec<_>>();
    Some(Gap {
        rank,
        title: cells[1].to_owned(),
        owner: cells[2].to_owned(),
        proof_lane: cells[3].to_owned(),
        sources,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("repo root")
        .to_path_buf()
}

fn owner_names(repo: &std::path::Path) -> BTreeSet<String> {
    let text = fs::read_to_string(repo.join(".jankurai/owner-map.json")).expect("owner map");
    let value: serde_json::Value = serde_json::from_str(&text).expect("owner map JSON");
    value
        .get("owners")
        .and_then(serde_json::Value::as_object)
        .expect("owners object")
        .values()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn proof_lane_names(repo: &std::path::Path) -> BTreeSet<String> {
    let text = fs::read_to_string(repo.join(".jankurai/proof-lanes.toml")).expect("proof lanes");
    let mut lanes = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("name = ") {
            lanes.insert(name.trim_matches('"').to_owned());
        } else if line.starts_with('[') && line.ends_with(']') && !line.starts_with("[[") {
            lanes.insert(line.trim_matches(['[', ']']).to_owned());
        }
    }
    lanes
}
