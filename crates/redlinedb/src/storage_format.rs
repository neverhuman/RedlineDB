use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, ErrorCode, Result};

/// Monotonic RedlineDB storage-format generation.
///
/// This identity is independent of crate/package SemVer. Compatible 4.x
/// releases retain generation 1. A future format change must advance this
/// value only with backward-read, migration, future-format rejection, and
/// rollback evidence.
pub const STORAGE_FORMAT_VERSION: u32 = 1;

/// Oldest storage-format generation this engine can read directly.
pub const MIN_READABLE_STORAGE_FORMAT_VERSION: u32 = 1;

/// Name of the authoritative generation marker in every durable database root.
pub const STORAGE_FORMAT_FILE_NAME: &str = "STORAGE_FORMAT";

const STORAGE_FORMAT_MAGIC: &str = "redlinedb-storage-format/v1\n";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StorageFormatState {
    Current,
    LegacyGenerationOne,
}

/// Returns whether a persisted storage generation is directly readable.
#[must_use]
pub const fn supports_storage_format(version: u32) -> bool {
    version >= MIN_READABLE_STORAGE_FORMAT_VERSION && version <= STORAGE_FORMAT_VERSION
}

pub(crate) fn inspect_storage_format(root: &Path) -> Result<StorageFormatState> {
    let path = root.join(STORAGE_FORMAT_FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(StorageFormatState::LegacyGenerationOne);
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "storage format marker is not a physical regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(Error::new(
                ErrorCode::Corrupt,
                "storage format marker has multiple hard links",
            ));
        }
    }

    let bytes = fs::read(&path)?;
    let generation = parse_storage_format(&bytes)?;
    if !supports_storage_format(generation) {
        return Err(Error::new(
            ErrorCode::Unsupported,
            format!("unsupported storage format generation {generation}"),
        ));
    }
    Ok(StorageFormatState::Current)
}

pub(crate) fn persist_current_storage_format(root: &Path) -> Result<()> {
    if inspect_storage_format(root)? == StorageFormatState::Current {
        return Ok(());
    }

    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = root.join(format!(
        ".{STORAGE_FORMAT_FILE_NAME}.tmp.{}.{sequence}",
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)?;
        file.write_all(storage_format_bytes().as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp, root.join(STORAGE_FORMAT_FILE_NAME))?;
        File::open(root)?.sync_all()?;
        inspect_storage_format(root)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn parse_storage_format(bytes: &[u8]) -> Result<u32> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        Error::new(
            ErrorCode::Corrupt,
            "storage format marker is not valid UTF-8",
        )
    })?;
    let generation = text
        .strip_prefix(STORAGE_FORMAT_MAGIC)
        .and_then(|rest| rest.strip_prefix("generation="))
        .and_then(|rest| rest.strip_suffix('\n'))
        .filter(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|rest| rest.parse::<u32>().ok())
        .ok_or_else(|| Error::new(ErrorCode::Corrupt, "malformed storage format marker"))?;
    if text != format!("{STORAGE_FORMAT_MAGIC}generation={generation}\n") {
        return Err(Error::new(
            ErrorCode::Corrupt,
            "non-canonical storage format marker",
        ));
    }
    Ok(generation)
}

fn storage_format_bytes() -> String {
    format!("{STORAGE_FORMAT_MAGIC}generation={STORAGE_FORMAT_VERSION}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_storage_formats_are_not_silently_accepted() {
        assert!(supports_storage_format(STORAGE_FORMAT_VERSION));
        assert!(!supports_storage_format(0));
        assert!(!supports_storage_format(STORAGE_FORMAT_VERSION + 1));
    }

    #[test]
    fn marker_parser_rejects_non_canonical_or_zero_generations() {
        assert_eq!(
            parse_storage_format(storage_format_bytes().as_bytes()).expect("current marker"),
            STORAGE_FORMAT_VERSION
        );
        for invalid in [
            b"generation=1\n".as_slice(),
            b"redlinedb-storage-format/v1\ngeneration=01\n".as_slice(),
            b"redlinedb-storage-format/v1\ngeneration=0\n".as_slice(),
            b"redlinedb-storage-format/v1\ngeneration=1".as_slice(),
        ] {
            let result = parse_storage_format(invalid).and_then(|generation| {
                if supports_storage_format(generation) {
                    Ok(())
                } else {
                    Err(Error::new(ErrorCode::Unsupported, "unsupported generation"))
                }
            });
            assert!(result.is_err(), "accepted invalid marker {invalid:?}");
        }
    }
}
