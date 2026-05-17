use super::{PageBackedHeap, RelationWriteTarget};

#[path = "write/append.rs"]
mod append;
#[path = "write/ops.rs"]
mod ops;
#[path = "write/recovery.rs"]
mod recovery;
