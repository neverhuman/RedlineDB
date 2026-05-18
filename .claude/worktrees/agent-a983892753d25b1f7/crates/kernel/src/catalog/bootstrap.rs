use std::sync::Arc;

use super::schema::{CatalogMeta, NamespaceDef, SchemaEpoch, SchemaSnapshot};
use super::{ObjectId, SchemaId};
use crate::format::RelId;

pub fn bootstrap_schema(next_relation_id: RelId) -> Arc<SchemaSnapshot> {
    let meta = CatalogMeta {
        format_version: 2,
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
