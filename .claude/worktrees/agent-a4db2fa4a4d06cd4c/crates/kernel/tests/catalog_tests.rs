use std::sync::Arc;

use redlinedb_kernel::catalog::SortDir;
use redlinedb_kernel::catalog::{
    Affinity, CatalogManager, OwnedValue, RecordRef, RecordScratch, SchemaEpoch, ValueRef,
    apply_affinity, bootstrap_schema, compare_index_keys, derive_affinity, encode_index_key,
    encode_record,
};
use redlinedb_kernel::format::RelId;

#[test]
fn affinity_derivation_and_coercion_match_sqlite_style_rules() {
    assert_eq!(derive_affinity(Some("INTEGER")), Affinity::Integer);
    assert_eq!(derive_affinity(Some("VARCHAR(20)")), Affinity::Text);
    assert_eq!(derive_affinity(Some("BLOB")), Affinity::Blob);
    assert_eq!(derive_affinity(Some("DOUBLE")), Affinity::Real);
    assert_eq!(derive_affinity(Some("NUMERIC")), Affinity::Numeric);

    assert_eq!(
        apply_affinity(OwnedValue::Integer(12), Affinity::Text).unwrap(),
        OwnedValue::Text(Arc::from("12"))
    );
    assert_eq!(
        apply_affinity(OwnedValue::Text(Arc::from("42")), Affinity::Integer).unwrap(),
        OwnedValue::Integer(42)
    );
}

#[test]
fn record_round_trips_and_rejects_truncation() {
    let mut encoded = Vec::new();
    encode_record(
        &[
            ValueRef::Null,
            ValueRef::Integer(7),
            ValueRef::Real(2.5),
            ValueRef::Text("hello"),
            ValueRef::Blob(b"world"),
        ],
        &mut encoded,
    )
    .unwrap();

    let record = RecordRef::new(&encoded).unwrap();
    let mut scratch = RecordScratch::default();
    record.decode_into(&mut scratch).unwrap();
    assert_eq!(record.value_at(&scratch, 0).unwrap(), ValueRef::Null);
    assert_eq!(record.value_at(&scratch, 1).unwrap(), ValueRef::Integer(7));
    assert_eq!(record.value_at(&scratch, 2).unwrap(), ValueRef::Real(2.5));
    assert_eq!(
        record.value_at(&scratch, 3).unwrap(),
        ValueRef::Text("hello")
    );
    assert_eq!(
        record.value_at(&scratch, 4).unwrap(),
        ValueRef::Blob(b"world")
    );

    let mut truncated = encoded.clone();
    truncated.pop();
    let record = RecordRef::new(&truncated).unwrap();
    assert!(record.decode_into(&mut scratch).is_err());
}

#[test]
fn index_keys_order_by_typed_prefixes() {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let a = encode_index_key(
        &[ValueRef::Integer(1), ValueRef::Text("abc")],
        &[SortDir::Asc, SortDir::Asc],
        &mut left,
    );
    let b = encode_index_key(
        &[ValueRef::Integer(2), ValueRef::Text("abc")],
        &[SortDir::Asc, SortDir::Asc],
        &mut right,
    );
    assert!(compare_index_keys(&a.bytes, &b.bytes).is_lt());
    assert!(!a.contains_null);
}

#[test]
fn catalog_manager_publishes_new_schema_epoch() {
    let initial = bootstrap_schema(RelId(10_000));
    let catalog = CatalogManager::new(Arc::clone(&initial));
    assert_eq!(catalog.version(), SchemaEpoch(1));
    let mut next = (*initial).clone();
    next.meta.schema_epoch = SchemaEpoch(2);
    let next = Arc::new(next);
    catalog.publish(Arc::clone(&next));
    assert_eq!(catalog.version(), SchemaEpoch(2));
    assert_eq!(catalog.current().meta.schema_epoch, SchemaEpoch(2));
}

#[test]
fn bootstrap_snapshot_contains_main_schema() {
    let snapshot = bootstrap_schema(RelId(10_000));
    assert_eq!(snapshot.namespaces.len(), 1);
    assert_eq!(snapshot.namespaces[0].name.as_ref(), "main");
}
