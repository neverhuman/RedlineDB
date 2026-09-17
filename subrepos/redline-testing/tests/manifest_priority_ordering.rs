//! Verify the pinned 1127-case parity manifest still loads in order. Extended
//! shards land in later commits and add their own coverage assertions; this
//! file holds the floor so the harness boots even before any shard is added.

#[test]
fn pinned_manifest_loads_and_starts_at_one() {
    let raw = include_str!("../corpus/sqlite_parity/generated_manifest.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("manifest parses");
    let arr = v.as_array().expect("manifest is an array");
    assert!(!arr.is_empty(), "pinned manifest is non-empty");
    let first = arr[0]["id"].as_u64().expect("first case has integer id");
    assert_eq!(first, 1, "pinned manifest starts at id=1");
    let last = arr.last().unwrap()["id"]
        .as_u64()
        .expect("last case has integer id");
    assert!(
        last >= first,
        "ids monotonically non-decreasing: first={first} last={last}"
    );
}
