//! Integration test: shape of the JSONL record contract.
//!
//! Every shipping record (parity or beyond) must carry a non-negative
//! `target_elapsed_ns` and a `memory_status` field from a fixed vocabulary.
//! These are the load-bearing fields the downstream RedlineDB report parser
//! depends on; this test pins them so a refactor of `report.rs` /
//! `evidence.rs` can't silently drop one.

use std::path::PathBuf;

fn raw_record_schema() -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("raw-record.schema.json");
    let raw = std::fs::read_to_string(&path).expect("read raw-record schema");
    serde_json::from_str(&raw).expect("parse raw-record schema")
}

#[test]
fn raw_record_schema_pins_required_fields() {
    let schema = raw_record_schema();
    let required = schema
        .get("required")
        .and_then(|v| v.as_array())
        .expect("schema required is an array");
    let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    // These exact field names are the public contract with RedlineDB's
    // report parser. Adding to the list is allowed; removing breaks downstream.
    for needed in [
        "case_id",
        "name",
        "priority",
        "profile",
        "category",
        "sample_role",
        "status",
        "reference_elapsed_ns",
        "target_elapsed_ns",
    ] {
        assert!(
            names.contains(&needed),
            "raw-record schema missing required field {needed}; current = {names:?}"
        );
    }
}

#[test]
fn memory_status_vocabulary_is_fixed() {
    let schema = raw_record_schema();
    let memory_status = schema
        .get("properties")
        .and_then(|p| p.get("memory_status"))
        .and_then(|m| m.get("enum"))
        .and_then(|e| e.as_array())
        .expect("memory_status.enum is an array");
    let values: Vec<&str> = memory_status.iter().filter_map(|v| v.as_str()).collect();
    for needed in ["sampled", "unavailable", "disabled", "not_run"] {
        assert!(
            values.contains(&needed),
            "memory_status enum missing {needed}; current = {values:?}"
        );
    }
}

#[test]
fn status_vocabulary_is_fixed() {
    let schema = raw_record_schema();
    let status = schema
        .get("properties")
        .and_then(|p| p.get("status"))
        .and_then(|m| m.get("enum"))
        .and_then(|e| e.as_array())
        .expect("status.enum is an array");
    let values: Vec<&str> = status.iter().filter_map(|v| v.as_str()).collect();
    for needed in ["passed", "failed", "skipped"] {
        assert!(
            values.contains(&needed),
            "status enum missing {needed}; current = {values:?}"
        );
    }
}

#[test]
fn elapsed_ns_fields_have_integer_type_and_nonneg_minimum() {
    let schema = raw_record_schema();
    for field in ["reference_elapsed_ns", "target_elapsed_ns"] {
        let prop = schema
            .get("properties")
            .and_then(|p| p.get(field))
            .unwrap_or_else(|| panic!("schema missing {field}"));
        let ty = prop
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| panic!("{field} missing type"));
        assert_eq!(ty, "integer", "{field} must be integer");
        let min = prop
            .get("minimum")
            .and_then(|m| m.as_i64())
            .unwrap_or_else(|| panic!("{field} missing minimum"));
        assert_eq!(min, 0, "{field} minimum must be 0");
    }
}
