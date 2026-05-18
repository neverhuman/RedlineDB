use crate::format::RelId;

pub const SYS_META: RelId = RelId(1);
pub const SYS_SCHEMAS: RelId = RelId(2);
pub const SYS_TABLES: RelId = RelId(3);
pub const SYS_COLUMNS: RelId = RelId(4);
pub const SYS_INDEXES: RelId = RelId(5);
pub const SYS_INDEX_COLUMNS: RelId = RelId(6);
pub const SYS_CONSTRAINTS: RelId = RelId(7);
pub const SYS_CONSTRAINT_COLUMNS: RelId = RelId(8);
pub const SYS_STATS: RelId = RelId(9);
pub const SYS_PENDING_DROP: RelId = RelId(10);

pub const CATALOG_FORMAT_VERSION: u64 = 1;
pub const MAIN_SCHEMA_ID: u64 = 1;
pub const CATALOG_REL_BASE: u64 = 10_000;
