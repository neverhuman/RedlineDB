//! Whole-database page checksum and LSN-monotonicity sweep.
//!
//! Every `Page` carries a CRC32 over its bytes (see
//! [`crate::format::checksum_page_bytes`]); `Page::from_bytes` already
//! verifies that checksum on every load, so this module re-uses the same
//! path through the buffer pool and converts pin failures into structured
//! [`super::PageCsumFailure`] entries.
//!
//! Two passes are interleaved here:
//!
//! 1. checksum verification — pin the page; on `Error::InvalidChecksum`,
//!    re-read the raw bytes through the page file and compute expected vs
//!    got so the report can name both numbers.
//! 2. LSN monotonicity — for each page id we record the page-LSN of the
//!    first successful read; if a later read in the same sweep observes a
//!    smaller value (which can only happen if the buffer pool returns a
//!    stale clone alongside a newer one) we surface a violation. In a
//!    single-pass walk this is a sanity invariant that should always hold;
//!    the field exists to harden the API for future concurrent walks.

use std::collections::HashMap;

use crate::format::{Lsn, Page, PageId, checksum_page_bytes};
use crate::{Error, Result};

use super::{IntegrityReport, LsnMonotonicityViolation, PageCsumFailure};

pub(crate) fn sweep(engine: &crate::engine::Engine, report: &mut IntegrityReport) -> Result<()> {
    // Lane E failpoint: armed before the integrity checker reads pages so
    // a failpoint-matrix case can later inject a transient buffer-pool
    // fault during the sweep.
    crate::fail_point!("integrity::page_csum::sweep");
    let buffer = engine.buffer_for_integrity();
    let page_count = buffer.page_count()?;
    let mut last_lsn: HashMap<PageId, Lsn> = HashMap::new();
    for page_no in 1..=page_count {
        let page_id = PageId(page_no);
        match buffer.pin(page_id) {
            Ok(guard) => {
                let lsn = guard.with_page(|page| Ok(page.header()?.page_lsn))?;
                if let Some(prev) = last_lsn.insert(page_id, lsn)
                    && lsn < prev
                {
                    report
                        .lsn_monotonicity_violations
                        .push(LsnMonotonicityViolation {
                            page_id,
                            prev_lsn: prev,
                            this_lsn: lsn,
                        });
                }
            }
            Err(Error::InvalidChecksum) => {
                if let Some(failure) = recompute_failure(engine, page_id) {
                    report.page_csum_failures.push(failure);
                } else {
                    report.page_csum_failures.push(PageCsumFailure {
                        page_id,
                        expected: 0,
                        got: 0,
                    });
                }
            }
            Err(Error::InvalidMagic { actual: 0, .. }) => {
                report.pages_skipped += 1;
            }
            Err(other) => {
                report
                    .errors
                    .push(format!("page {} pin failed: {}", page_id.0, other));
            }
        }
    }
    Ok(())
}

/// Re-read raw bytes for a page that failed `pin`'s checksum verification
/// so the report can name the expected vs got CRC32. Falls back to None if
/// even the raw read fails (e.g. truncated file) — caller emits a
/// zero-zero placeholder in that case.
fn recompute_failure(engine: &crate::engine::Engine, page_id: PageId) -> Option<PageCsumFailure> {
    let bytes = engine.read_raw_page_bytes_for_integrity(page_id).ok()?;
    if bytes.len() < crate::format::PAGE_HEADER_LEN {
        return None;
    }
    let stored = match crate::format::bytes::read_u32(&bytes, page_checksum_offset()) {
        Ok(value) => value,
        Err(_) => return None,
    };
    let got = checksum_page_bytes(&bytes);
    Some(PageCsumFailure {
        page_id,
        expected: stored,
        got,
    })
}

fn page_checksum_offset() -> usize {
    // Mirror the constant in `format::page` (kept private there). The
    // checksum field sits at byte 8 of every page; this helper makes the
    // dependency explicit instead of leaking a private const.
    8
}

#[allow(dead_code)]
fn _force_use(_p: Page) {}
