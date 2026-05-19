//! Public API surface for the `redlinedb` embedded database crate.
//!
//! This module is the public-facing facade: it declares the sub-modules,
//! re-exports the user-visible types/traits, and keeps the rest of the
//! implementation factored into focused files (`handle.rs`, `connection.rs`,
//! `statement.rs`, `iter.rs`, plus the existing `error`, `options`,
//! `params`, `phase8`, `registry`, `snapshot`, `value`, `machine`).

mod connection;
mod error;
mod handle;
mod iter;
mod machine;
mod options;
mod params;
mod phase8;
mod pool;
mod registry;
mod snapshot;
mod statement;
mod value;

pub mod metrics;

#[cfg(feature = "tokio")]
mod asyncio;

#[cfg(feature = "tokio")]
pub use asyncio::{AsyncConnection, AsyncDatabase};

pub use pool::{Pool, PoolBuilder, PooledConnection};

pub use connection::{Connection, InterruptHandle, Transaction};
pub use error::{Error, ErrorCode, Result};
pub use handle::Database;
pub use iter::{FromRow, FromValue, OwnedStep, Row, Step};
pub use machine::{
    BinaryOp, ColumnRef, DeleteSpec, ExprSpec, InsertSpec, OrderSpec, QuerySpec, SchemaHandle,
    SelectSpec, TableRef, UnaryOp, UpdateSpec,
};
pub use options::{
    AnalyzeOptions, BackupOptions, BackupStats, BenchmarkStats, BufferStats, CheckpointBenchStats,
    CheckpointStats, CommitStats, ConnectionStats, DatabaseStats, Durability, ExecuteSummary,
    FunctionArity, FunctionFlags, MemoryOptions, OpenOptions, OptimizerOptions, QueryMemoryOptions,
    TxBenchStats, VacuumStats, WalBenchStats,
};
pub use params::Params;
pub use phase8::{
    ArchiveMode, ArchiveStats, PhysicalBackupOptions, PhysicalBackupStats, ReplicationSlot,
    ReplicationSlotStats, RestoreOptions, RestoreStats, RetentionHorizon, SlotKind, WalLevel,
};
pub use redlinedb_kernel::format::{BackupId, Csn, DbId, Lsn, TimelineId, WalSegmentNo};
pub use redlinedb_sql::BeginMode;
pub use redlinedb_sql::RecoveryTarget;
pub use statement::{OwnedStatement, Prepared, Rows, Statement};
pub use value::{Value, ValueRef};

// `registry::open_database` and friends call `crate::sql_options`; keep the
// path stable by re-exporting the implementation hosted in `handle`.
pub(crate) use handle::sql_options;
