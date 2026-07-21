/// Monotonic RedlineDB storage-format generation.
///
/// This identity is independent of crate/package SemVer. Compatible 4.x
/// releases retain generation 1. A future format change must advance this
/// value only with backward-read, migration, future-format rejection, and
/// rollback evidence.
pub const STORAGE_FORMAT_VERSION: u32 = 1;

/// Oldest storage-format generation this engine can read directly.
pub const MIN_READABLE_STORAGE_FORMAT_VERSION: u32 = 1;

/// Returns whether a persisted storage generation is directly readable.
#[must_use]
pub const fn supports_storage_format(version: u32) -> bool {
    version >= MIN_READABLE_STORAGE_FORMAT_VERSION && version <= STORAGE_FORMAT_VERSION
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
}
