//! Heap/index/page integrity checker for RedlineDB.
//!
//! Lane INT lifts the previous "stub returns ok" `PRAGMA integrity_check`
//! into a real cross-validation of the durable state:
//!
//! * heap walk: every visible row in every relation,
//! * index walk: every live leaf entry on every catalog index,
//! * equivalence: heap rows == index entries (per index),
//! * page checksums: every persisted page's CRC32 matches its bytes,
//! * lsn monotonicity: page-LSNs never travel backwards on a page id.
//!
//! The checker is read-only: it pins pages and reads them through the buffer
//! pool but never mutates anything. SQL exposes two PRAGMAs that consume
//! [`IntegrityReport`]:
//! `PRAGMA redline_index_check` and `PRAGMA redline_full_check`.

pub mod equivalence;
pub mod heap;
pub mod index;
pub mod page_csum;

use crate::catalog::IndexId as CatalogIndexId;
use crate::format::{Lsn, PageId, RelId};

/// Outcome of [`crate::engine::Engine::integrity_check_full`]. One
/// [`RelationReport`] is emitted per relation; per-database state (page
/// checksums, LSN monotonicity) lives at the top level so callers can
/// summarise without re-walking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntegrityReport {
    pub relations: Vec<RelationReport>,
    pub page_csum_failures: Vec<PageCsumFailure>,
    pub lsn_monotonicity_violations: Vec<LsnMonotonicityViolation>,
    /// Pages skipped because they do not yet exist on disk (pin returned a
    /// zero-magic load). These are legitimate sparse-allocation cases and
    /// not anomalies in themselves.
    pub pages_skipped: usize,
    /// Top-level errors that don't fit the per-relation slots — usually
    /// catalog drift or buffer-pool faults that prevented progress.
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelationReport {
    pub relation_id: RelId,
    pub relation_name: String,
    pub heap_row_count: usize,
    pub indexes: Vec<IndexReport>,
    /// Per-relation error pile (e.g. a row whose record decoded badly while
    /// we tried to derive its index keys for cross-check).
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub index_id: CatalogIndexId,
    pub index_name: String,
    pub entry_count: usize,
    /// Number of heap rows whose computed key+row_id has no matching live
    /// index entry. Healthy state: 0.
    pub heap_minus_index: usize,
    /// Number of live index entries whose row_id does not resolve to a
    /// visible heap row (or whose row_id resolves to a row whose key has
    /// drifted away from the index entry's key). Healthy state: 0.
    pub index_minus_heap: usize,
    /// Validation-level structural issues surfaced by [`crate::index::BtreeIndex::validate`]
    /// (page kind / level / id mismatches). Translated to owned `String` so
    /// the report can cross thread / FFI boundaries without `'static` ties.
    pub structural_errors: Vec<String>,
    /// Other index-specific anomalies (e.g. duplicate `(key, row_id)` in a
    /// unique index).
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCsumFailure {
    pub page_id: PageId,
    pub expected: u32,
    pub got: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LsnMonotonicityViolation {
    pub page_id: PageId,
    /// LSN observed on the previous read of this page within the same
    /// integrity walk.
    pub prev_lsn: Lsn,
    /// LSN observed on the current read.
    pub this_lsn: Lsn,
}

impl IntegrityReport {
    /// Returns true when every probed relation, index, and page is healthy:
    /// no orphans, no checksum failures, no monotonicity violations, no
    /// errors. Used by the `redline_full_check` PRAGMA to render a single
    /// "ok" / "errors" summary per relation.
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
            && self.page_csum_failures.is_empty()
            && self.lsn_monotonicity_violations.is_empty()
            && self.relations.iter().all(|r| {
                r.errors.is_empty()
                    && r.indexes.iter().all(|i| {
                        i.heap_minus_index == 0
                            && i.index_minus_heap == 0
                            && i.structural_errors.is_empty()
                            && i.errors.is_empty()
                    })
            })
    }
}

/// Per-relation context shared between heap, index, and equivalence passes.
/// Built once by [`crate::engine::Engine::integrity_check_full`] and threaded
/// through the sub-modules so each can pin the buffer pool / read the
/// schema snapshot without recomputing it.
pub(crate) struct CheckContext<'a> {
    pub engine: &'a crate::engine::Engine,
}

/// Drive a full integrity sweep: per-relation heap walk, per-index leaf
/// dump, equivalence cross-check, then a database-wide page checksum and
/// LSN-monotonicity sweep. Public entry is
/// [`crate::engine::Engine::integrity_check_full`]; this function exists
/// at module scope so the sub-modules can each focus on one concern.
pub(crate) fn run_full(engine: &crate::engine::Engine) -> crate::Result<IntegrityReport> {
    let schema = engine.schema_snapshot();
    let ctx = CheckContext { engine };
    let mut report = IntegrityReport::default();

    let handles = engine.index_handles_for_integrity()?;

    for table in &schema.tables {
        let mut rel_report = RelationReport {
            relation_id: table.relation_id,
            relation_name: table.name.to_string(),
            ..RelationReport::default()
        };

        let heap_rows = match heap::collect_visible_rows(&ctx, table.relation_id) {
            Ok(rows) => rows,
            Err(err) => {
                rel_report.errors.push(format!("heap walk failed: {err}"));
                report.relations.push(rel_report);
                continue;
            }
        };
        rel_report.heap_row_count = heap_rows.len();

        for catalog_index in &schema.indexes {
            if catalog_index.table_id != table.table_id {
                continue;
            }
            let mut index_report = IndexReport {
                index_id: catalog_index.index_id,
                index_name: catalog_index.name.to_string(),
                ..IndexReport::default()
            };

            let Some(btree) = handles.get(&catalog_index.index_id) else {
                // Pre-Lane-A indexes have no physical pages; nothing to check.
                rel_report.indexes.push(index_report);
                continue;
            };

            let dump = match index::dump_index(btree) {
                Ok(dump) => dump,
                Err(err) => {
                    index_report
                        .errors
                        .push(format!("index dump failed: {err}"));
                    rel_report.indexes.push(index_report);
                    continue;
                }
            };
            index_report.entry_count = dump.entries.len();
            index_report.structural_errors = dump
                .validation
                .errors
                .iter()
                .map(|s| (*s).to_owned())
                .collect();

            if let Err(err) = equivalence::cross_check(
                table.as_ref(),
                catalog_index.as_ref(),
                &heap_rows,
                &dump.entries,
                &mut index_report,
            ) {
                index_report
                    .errors
                    .push(format!("equivalence check failed: {err}"));
            }

            rel_report.indexes.push(index_report);
        }

        report.relations.push(rel_report);
    }

    if let Err(err) = page_csum::sweep(engine, &mut report) {
        report
            .errors
            .push(format!("page checksum sweep failed: {err}"));
    }

    Ok(report)
}
