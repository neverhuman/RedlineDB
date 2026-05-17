//! Catalog, DDL, and physical index handle lifecycle operations.

use std::sync::Arc;

use crate::catalog::{
    IndexId as CatalogIndexId, SchemaEpoch, SqliteSchemaRow, apply_alter_table, apply_create_index,
    apply_create_table, apply_drop_index, apply_drop_table, apply_set_index_meta_page_id,
    lookup_table,
};
use crate::engine::tx::PendingIndexHandle;
use crate::format::{PageGeneration, PageId, TuplePtr};
use crate::index::{
    BtreeIndex, INDEX_VERSION, IndexDescriptor, IndexId as PhysicalIndexId, IndexRowRef,
    IndexUniqueness,
};
use crate::txn::Isolation;
use crate::{Error, Result};

use super::{CommitOutcome, Engine, Txn};

#[path = "catalog_ops/index.rs"]
mod index;
#[path = "catalog_ops/schema.rs"]
mod schema;
#[path = "catalog_ops/support.rs"]
mod support;
