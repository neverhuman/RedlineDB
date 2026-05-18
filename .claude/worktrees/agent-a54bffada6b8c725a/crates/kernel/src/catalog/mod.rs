mod affinity;
mod bootstrap;
mod ddl;
mod expr;
mod ids;
mod key;
mod manager;
mod names;
mod ops;
mod record;
mod schema;
mod stats;
mod store;
mod system;
mod value;

pub use affinity::{Affinity, CoerceError, apply_affinity, derive_affinity};
pub use bootstrap::bootstrap_schema;
pub use ddl::{
    AlterTableOperationSpec, AlterTableSpec, ColumnConstraintSpec, ColumnSpec, ConflictAction,
    CreateIndexSpec, CreateTableSpec, DropIndexSpec, DropTableSpec, IndexColumnSpec, IndexOrigin,
    TableConstraintSpec,
};
pub use expr::{
    CompiledExpr, EvalScratch, ExprAst, ExprError, ExprOp, RowValueSource, compile_expr, eval_expr,
};
pub use ids::{ColumnId, ConstraintId, IndexId, ObjectId, RelationId, SchemaId, TableId};
pub use key::{
    EncodedIndexKey, IndexKeyDef, IndexKeySource, NullOrder, SortDir, compare_index_keys,
    encode_index_key,
};
pub use manager::CatalogManager;
pub use names::{DbName, QualifiedName};
pub use ops::{
    apply_alter_table, apply_create_index, apply_create_table, apply_drop_index, apply_drop_table,
    apply_set_index_meta_page_id, lookup_index, lookup_table, resolve_schema_id,
};
pub use record::{RecordRef, RecordScratch, encode_record};
pub use schema::{
    CatalogError, CatalogMeta, CheckDef, ClassKind, ColumnDef, ConstraintDef, ConstraintKind,
    IndexDef, NamespaceDef, SchemaEpoch, SchemaSnapshot, SqliteSchemaRow, TableDef,
};
pub use stats::{
    ColumnStats, HistogramBucket, IndexStats, MostCommonValue, StatsEpoch, StatsSnapshot,
    StatsStore, TableStats,
};
pub use store::{CatalogStore, decode_snapshot, encode_snapshot};
pub use system::*;
pub use value::{OwnedValue, StorageClass, ValueRef};
