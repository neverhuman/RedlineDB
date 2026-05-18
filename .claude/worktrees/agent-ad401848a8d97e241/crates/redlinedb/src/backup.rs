use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::Database;
use crate::error::Result;
use crate::options::{BackupOptions, BackupStats};

pub(crate) fn backup_to_path(
    src: &Database,
    dst: impl AsRef<Path>,
    _options: BackupOptions,
) -> Result<BackupStats> {
    let start = Instant::now();
    let dst = dst.as_ref();
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    src.inner.db.checkpoint()?;
    copy_dir(src.path(), dst)?;
    Ok(BackupStats {
        tables_copied: 0,
        rows_copied: 0,
        indexes_created: 0,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "owner.lock" {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
