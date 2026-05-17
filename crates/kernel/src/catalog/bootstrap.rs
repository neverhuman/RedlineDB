use std::sync::Arc;

use super::schema::{CatalogMeta, NamespaceDef, SchemaEpoch, SchemaSnapshot};
use super::{ObjectId, SchemaId};
use crate::format::RelId;

pub fn bootstrap_schema(next_relation_id: RelId) -> Arc<SchemaSnapshot> {
    let meta = CatalogMeta {
        // v5 adds per-table foreign_keys vector (A6 SQLite parity FK
        // enforcement) and the per-snapshot views section (A5-views
        // SQLite parity). v6 adds the triggers section (A5-triggers
        // SQLite parity). Older catalogs decode with empty FK / view /
        // trigger lists, so the bumps are forward-compatible.
        format_version: 6,
        schema_epoch: SchemaEpoch(1),
        next_object_id: ObjectId(10_000),
        next_relation_id,
        database_uuid: *b"RedlineDBPhase4!",
    };
    let mut snapshot = SchemaSnapshot::empty(meta);
    snapshot.namespaces.push(NamespaceDef {
        schema_id: SchemaId(1),
        name: "main".into(),
        folded: "main".into(),
    });
    snapshot.rebuild_indexes();
    Arc::new(snapshot)
}
