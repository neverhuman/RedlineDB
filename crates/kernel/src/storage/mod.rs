pub mod buffer;
pub mod control;
pub mod page_file;
pub mod tx_status_checkpoint;

pub use buffer::*;
pub use control::*;
pub use page_file::*;
pub use tx_status_checkpoint::*;

use std::fs::File;
use std::path::Path;

use crate::Result;

/// Open the directory at `path` and `sync_data` it so a preceding rename
/// or create-and-close becomes durable on POSIX. Shared by every storage
/// submodule that touches the database directory atomically; prior to the
/// dedup pass each submodule carried its own private copy of this body.
#[inline]
pub(crate) fn sync_parent_dir(path: &Path) -> Result<()> {
    let file = File::open(path)?;
    file.sync_data()?;
    Ok(())
}
