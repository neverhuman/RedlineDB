use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};

use smallvec::SmallVec;

use crate::format::bytes::{read_u16, read_u32, read_u64, write_u16, write_u32, write_u64};
use crate::format::{Page, PageGeneration, PageId, PageKind, RelId, TuplePtr, TxId};
use crate::storage::BufferPool;
use crate::wal::{WalCoordinator, WalPayload, WalRecordKind};
use crate::{Error, Result};

mod cells;
mod cursor;
mod locks;
mod lookup;
mod maintenance;
mod mutate;
mod scan;

use cells::{Entry, InternalCell, LeafCell, LeafEntry};

pub use cursor::{CursorYield, IndexCursor, KeyRange, RawIndexCursor, SnapshotView};
pub use locks::{UniqueKeyGuard, UniqueKeyLockTable};

pub const INDEX_SPECIAL_LEN: usize = 256;
const INDEX_MAGIC: u32 = 0x5244_4958; // "RDIX"
pub const INDEX_VERSION: u16 = 2;
const PAGE_META_KIND: u8 = 1;
pub(crate) const PAGE_LEAF_KIND: u8 = 2;
pub(crate) const PAGE_INTERNAL_KIND: u8 = 3;
const META_ROOT_PAGE_OFF: usize = 16;
const META_ROOT_LEVEL_OFF: usize = 24;
const META_UNIQUENESS_OFF: usize = 26;
const META_HIGH_KEY_LEN_OFF: usize = 32;
const META_HIGH_KEY_OFF: usize = 34;
const PAGE_LEVEL_OFF: usize = 8;
const PAGE_INDEX_ID_OFF: usize = 10;
const PAGE_LEFT_OFF: usize = 18;
const PAGE_RIGHT_OFF: usize = 26;
const PAGE_HIGH_KEY_LEN_OFF: usize = 34;
const PAGE_HIGH_KEY_OFF: usize = 36;
pub(crate) const NON_TRANSACTIONAL_DELETE_TX: TxId = TxId(u64::MAX);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexId(pub u64);

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexUniqueness {
    NonUnique = 0,
    Unique = 1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexDescriptor {
    pub index_id: IndexId,
    pub rel_id: RelId,
    pub uniqueness: IndexUniqueness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IndexRowRef {
    pub row_id: crate::format::RowId,
    pub tuple: TuplePtr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBuf {
    bytes: SmallVec<[u8; 96]>,
    logical_len: usize,
}

impl KeyBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.bytes.clear();
        self.logical_len = 0;
    }

    pub fn extend_logical(&mut self, key: &[u8]) {
        self.logical_len = key.len();
        self.bytes.extend_from_slice(key);
    }

    pub fn append_row_ref_suffix(&mut self, row: IndexRowRef) {
        self.bytes.extend_from_slice(&row.row_id.0.to_be_bytes());
        self.bytes
            .extend_from_slice(&row.tuple.page_id.0.to_be_bytes());
        self.bytes.extend_from_slice(&row.tuple.slot.to_be_bytes());
        self.bytes
            .extend_from_slice(&row.tuple.generation.0.to_be_bytes());
    }

    pub fn logical_len(&self) -> usize {
        self.logical_len
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

impl Default for KeyBuf {
    fn default() -> Self {
        Self {
            bytes: SmallVec::new(),
            logical_len: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexValidationReport {
    pub pages_seen: usize,
    pub leaf_pages: usize,
    pub internal_pages: usize,
    pub errors: Vec<&'static str>,
}

/// Live (non-dead) leaf entry surfaced by [`BtreeIndex::iter_all_entries`].
/// Lane INT consumes this to cross-check every index entry against the heap
/// row directory; the integrity checker has no business inspecting tombstones
/// (those represent rolled-back or vacuumed deletions, not durable state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub logical_key: Vec<u8>,
    pub row: IndexRowRef,
    pub leaf_page_id: PageId,
}

#[derive(Clone, Debug)]
pub(super) struct MetaHeader {
    pub(super) index_id: IndexId,
    pub(super) root_page_id: PageId,
    pub(super) root_level: u16,
    pub(super) uniqueness: IndexUniqueness,
}

#[derive(Clone, Debug)]
pub(super) struct PageHeader {
    pub(super) kind: u8,
    pub(super) level: u16,
    pub(super) index_id: IndexId,
    pub(super) left: Option<PageId>,
    pub(super) right: Option<PageId>,
    pub(super) high_key: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct IndexInner {
    pub(super) buffer: Arc<BufferPool>,
    pub(super) meta_page_id: PageId,
    pub(super) desc: Mutex<IndexDescriptor>,
    pub(super) unique_locks: Arc<UniqueKeyLockTable>,
    pub(super) wal: Option<Arc<WalCoordinator>>,
    pub(super) structure_lock: Mutex<()>,
    /// Lifetime counter of leaf pages pinned by `range_scan`. Updated even
    /// when the feature gate is off so `BtreeIndex::stats()` is callable
    /// in any build, then consumed by Lane KH P1 #6 tests to assert the
    /// scan terminates as soon as the next leaf's first key falls outside
    /// the requested upper bound.
    pub(super) range_scan_leaves_visited: AtomicU64,
}

/// Per-index runtime counters surfaced for tests and observability.
/// Currently only `range_scan_leaves_visited` is wired; future waves can
/// extend this with point-lookup chain length, split counts, etc.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexStats {
    pub range_scan_leaves_visited: u64,
}

#[derive(Clone, Debug)]
pub struct BtreeIndex {
    pub(super) inner: Arc<IndexInner>,
}

impl BtreeIndex {
    pub fn create(buffer: Arc<BufferPool>, desc: IndexDescriptor) -> Result<Self> {
        Self::create_with_wal(buffer, desc, None)
    }

    pub fn create_with_wal(
        buffer: Arc<BufferPool>,
        desc: IndexDescriptor,
        wal: Option<Arc<WalCoordinator>>,
    ) -> Result<Self> {
        let meta_guard = buffer.allocate(PageKind::BtreeMeta, desc.rel_id)?;
        let root_guard = buffer.allocate(PageKind::BtreeLeaf, desc.rel_id)?;

        meta_guard.with_page_mut(|page| {
            page.reinitialize_with_special(
                PageKind::BtreeMeta,
                meta_guard.page_id(),
                desc.rel_id,
                PageGeneration::ONE,
                INDEX_SPECIAL_LEN,
            )?;
            Self::write_meta(
                page,
                &MetaHeader {
                    index_id: desc.index_id,
                    root_page_id: root_guard.page_id(),
                    root_level: 0,
                    uniqueness: desc.uniqueness,
                },
            )
        })?;
        // Keep newly-created WAL-backed index pages unflushable until the
        // create path records real page images for the DDL transaction.
        let create_lsn = if wal.is_some() {
            crate::format::Lsn(u64::MAX)
        } else {
            crate::format::Lsn(1)
        };
        meta_guard.mark_dirty(create_lsn)?;
        root_guard.with_page_mut(|page| {
            page.reinitialize_with_special(
                PageKind::BtreeLeaf,
                root_guard.page_id(),
                desc.rel_id,
                PageGeneration::ONE,
                INDEX_SPECIAL_LEN,
            )?;
            Self::write_page_header(
                page,
                &PageHeader {
                    kind: PAGE_LEAF_KIND,
                    level: 0,
                    index_id: desc.index_id,
                    left: None,
                    right: None,
                    high_key: Vec::new(),
                },
            )
        })?;
        root_guard.mark_dirty(create_lsn)?;

        Ok(Self {
            inner: Arc::new(IndexInner {
                buffer,
                meta_page_id: meta_guard.page_id(),
                desc: Mutex::new(desc),
                unique_locks: Arc::new(UniqueKeyLockTable::new(128)),
                wal,
                structure_lock: Mutex::new(()),
                range_scan_leaves_visited: AtomicU64::new(0),
            }),
        })
    }

    pub fn open(
        buffer: Arc<BufferPool>,
        meta_page_id: PageId,
        desc: IndexDescriptor,
    ) -> Result<Self> {
        Self::open_with_wal(buffer, meta_page_id, desc, None)
    }

    pub fn open_with_wal(
        buffer: Arc<BufferPool>,
        meta_page_id: PageId,
        desc: IndexDescriptor,
        wal: Option<Arc<WalCoordinator>>,
    ) -> Result<Self> {
        let index = Self {
            inner: Arc::new(IndexInner {
                buffer,
                meta_page_id,
                desc: Mutex::new(desc),
                unique_locks: Arc::new(UniqueKeyLockTable::new(128)),
                wal,
                structure_lock: Mutex::new(()),
                range_scan_leaves_visited: AtomicU64::new(0),
            }),
        };
        index.validate()?;
        Ok(index)
    }

    pub fn format_version(buffer: &BufferPool, meta_page_id: PageId) -> Result<u16> {
        let guard = buffer.pin(meta_page_id)?;
        guard.with_page(|page| {
            let special = page.special_bytes()?;
            if read_u32(special, 0)? != INDEX_MAGIC {
                return Err(Error::CorruptPage("index magic mismatch"));
            }
            read_u16(special, 4)
        })
    }

    pub fn lock_unique_key(&self, owner: u64, logical_key: &[u8]) -> Result<UniqueKeyGuard> {
        self.inner.unique_locks.lock(logical_key, owner)
    }

    pub fn redo_page_image(&self, page: Page) -> Result<()> {
        self.inner.buffer.write_page_direct(&page)?;
        let page_id = page.header()?.page_id;
        if let Ok(guard) = self.inner.buffer.pin(page_id) {
            guard.with_page_mut(|resident| {
                *resident = page.clone();
                Ok(())
            })?;
            guard.mark_dirty(page.header()?.page_lsn)?;
        }
        Ok(())
    }

    pub(super) fn record_page_image(
        &self,
        page_id: PageId,
        tx_id: crate::format::TxId,
    ) -> Result<()> {
        let guard = self.inner.buffer.pin(page_id)?;
        if tx_id == crate::format::TxId::ZERO {
            return guard.mark_dirty(crate::format::Lsn(1));
        }
        let Some(wal) = &self.inner.wal else {
            return guard.mark_dirty(crate::format::Lsn(1));
        };
        let mut page = guard.with_page(|page| Ok(page.clone()))?;
        page.set_page_lsn(crate::format::Lsn::ZERO)?;
        let payload = WalPayload::PageImage {
            page_id,
            page_lsn: crate::format::Lsn::ZERO,
            page_bytes: page.as_bytes().to_vec(),
        };
        let append = wal.append(WalRecordKind::PageImage, tx_id, payload.encode()?)?;
        guard.mark_dirty(append.end_lsn)
    }

    pub(super) fn append_index_delta(
        &self,
        tx_id: crate::format::TxId,
        payload: WalPayload,
    ) -> Result<crate::format::Lsn> {
        let Some(wal) = &self.inner.wal else {
            return Ok(crate::format::Lsn(1));
        };
        let append = wal.append(WalRecordKind::PageDelta, tx_id, payload.encode()?)?;
        Ok(append.end_lsn)
    }

    /// Returns a snapshot of per-index runtime counters. Lane KH P1 #6
    /// tests use `range_scan_leaves_visited` to assert the early-exit
    /// path is hit; recovery and benches can sample this for
    /// observability.
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            range_scan_leaves_visited: self
                .inner
                .range_scan_leaves_visited
                .load(AtomicOrdering::Relaxed),
        }
    }

    pub fn descriptor(&self) -> IndexDescriptor {
        self.inner
            .desc
            .lock()
            .expect("descriptor mutex poisoned")
            .clone()
    }

    /// Returns the meta-page id for this B-tree. Lane A persists this in the
    /// catalog snapshot so recovery can reopen the index handle.
    pub fn meta_page_id(&self) -> PageId {
        self.inner.meta_page_id
    }

    /// Log WAL `PageImage` records for the meta and root pages of a freshly
    /// created index so recovery can reconstruct the B-tree even if the
    /// engine drops before a checkpoint flushes the buffer pool. Caller must
    /// supply a real (non-zero) `TxId` whose commit LSN covers these images.
    pub fn record_initial_page_images(&self, tx_id: crate::format::TxId) -> Result<()> {
        let meta_id = self.inner.meta_page_id;
        let root_id = self.meta()?.root_page_id;
        self.record_page_image(meta_id, tx_id)?;
        self.record_page_image(root_id, tx_id)?;
        Ok(())
    }

    pub(super) fn find_leaf(&self, page_id: PageId, key: &[u8]) -> Result<PageId> {
        Ok(*self
            .find_leaf_path(page_id, key)?
            .last()
            .ok_or(Error::CorruptPage("empty search path"))?)
    }

    pub(super) fn find_leaf_path(&self, mut page_id: PageId, key: &[u8]) -> Result<Vec<PageId>> {
        let mut path = Vec::new();
        loop {
            path.push(page_id);
            let guard = self.inner.buffer.pin(page_id)?;
            let next = guard.with_page(|page| {
                let header = Self::read_page_header(page)?;
                if !header.high_key.is_empty() && key >= header.high_key.as_slice() {
                    return Ok(header.right);
                }
                if header.kind == PAGE_LEAF_KIND {
                    return Ok(None);
                }
                let mut chosen = header.left;
                if chosen.is_none() {
                    return Err(Error::CorruptPage("internal page missing leftmost child"));
                }
                for entry in self.read_entries(page)? {
                    if let Entry::Internal { separator, child } = entry {
                        if key < separator.as_slice() {
                            break;
                        }
                        chosen = Some(child);
                    }
                }
                Ok(chosen)
            })?;
            match next {
                Some(next_id) if next_id != page_id => page_id = next_id,
                _ => return Ok(path),
            }
        }
    }

    pub(super) fn meta(&self) -> Result<MetaHeader> {
        let guard = self.inner.buffer.pin(self.inner.meta_page_id)?;
        guard.with_page(Self::read_meta)
    }

    pub(super) fn set_meta_root(
        &self,
        root_page_id: PageId,
        root_level: u16,
        tx_id: crate::format::TxId,
        emit_wal: bool,
        lsn: crate::format::Lsn,
    ) -> Result<()> {
        let guard = self.inner.buffer.pin(self.inner.meta_page_id)?;
        guard.with_page_mut(|page| {
            let mut meta = Self::read_meta(page)?;
            meta.root_page_id = root_page_id;
            meta.root_level = root_level;
            Self::write_meta(page, &meta)
        })?;
        if emit_wal {
            self.record_page_image(self.inner.meta_page_id, tx_id)
        } else {
            guard.mark_dirty(lsn)
        }
    }

    pub(in crate::index) fn read_entries(&self, page: &Page) -> Result<Vec<Entry>> {
        let header = page.header()?;
        let mut entries = Vec::new();
        for slot in 0..page.slot_count()? {
            let cell = page.cell(slot)?;
            match header.kind {
                PageKind::BtreeLeaf => entries.push(LeafCell::decode(cell)?),
                PageKind::BtreeInternal => entries.push(InternalCell::decode(cell)?),
                _ => return Err(Error::CorruptPage("unsupported index page kind")),
            }
        }
        Ok(entries)
    }

    pub(in crate::index) fn read_leaf_entries(&self, page: &Page) -> Result<Vec<LeafEntry>> {
        let header = page.header()?;
        let mut entries = Vec::new();
        for slot in 0..page.slot_count()? {
            let cell = page.cell(slot)?;
            match header.kind {
                PageKind::BtreeLeaf => entries.push(LeafCell::decode_leaf_entry(cell)?),
                PageKind::BtreeInternal => {
                    return Err(Error::CorruptPage("unsupported index page kind"));
                }
                _ => return Err(Error::CorruptPage("unsupported index page kind")),
            }
        }
        Ok(entries)
    }

    pub(in crate::index) fn rewrite_leaf(
        page: &mut Page,
        index_id: IndexId,
        entries: &[Entry],
        left: Option<PageId>,
        right: Option<PageId>,
        high_key: Vec<u8>,
    ) -> Result<()> {
        let page_id = page.header()?.page_id;
        let rel_id = page.header()?.rel_id;
        page.reinitialize_with_special(
            PageKind::BtreeLeaf,
            page_id,
            rel_id,
            PageGeneration::ONE,
            INDEX_SPECIAL_LEN,
        )?;
        Self::write_page_header(
            page,
            &PageHeader {
                kind: PAGE_LEAF_KIND,
                level: 0,
                index_id,
                left,
                right,
                high_key,
            },
        )?;
        for entry in entries {
            if let Entry::Leaf {
                logical_key,
                row,
                physical,
                create_tx,
                delete_tx,
            } = entry
            {
                page.insert_cell(&LeafCell::encode(
                    logical_key,
                    *row,
                    physical,
                    *create_tx,
                    *delete_tx,
                ))?;
            }
        }
        Ok(())
    }

    pub(in crate::index) fn rewrite_internal(
        page: &mut Page,
        index_id: IndexId,
        level: u16,
        entries: &[Entry],
        left: Option<PageId>,
        right: Option<PageId>,
        high_key: Vec<u8>,
    ) -> Result<()> {
        let page_id = page.header()?.page_id;
        let rel_id = page.header()?.rel_id;
        page.reinitialize_with_special(
            PageKind::BtreeInternal,
            page_id,
            rel_id,
            PageGeneration::ONE,
            INDEX_SPECIAL_LEN,
        )?;
        Self::write_page_header(
            page,
            &PageHeader {
                kind: PAGE_INTERNAL_KIND,
                level,
                index_id,
                left,
                right,
                high_key,
            },
        )?;
        for entry in entries {
            if let Entry::Internal { separator, child } = entry {
                page.insert_cell(&InternalCell::encode(separator, *child))?;
            }
        }
        Ok(())
    }

    pub(super) fn read_page_header(page: &Page) -> Result<PageHeader> {
        let special = page.special_bytes()?;
        if special.len() < META_HIGH_KEY_OFF {
            return Err(Error::CorruptPage("index special header too small"));
        }
        let kind = special[6];
        let level = read_u16(special, PAGE_LEVEL_OFF)?;
        let index_id = IndexId(read_u64(special, PAGE_INDEX_ID_OFF)?);
        let left = decode_opt_page_id(read_u64(special, PAGE_LEFT_OFF)?);
        let right = decode_opt_page_id(read_u64(special, PAGE_RIGHT_OFF)?);
        let high_key_len = read_u16(special, PAGE_HIGH_KEY_LEN_OFF)? as usize;
        let high_key = if high_key_len == 0 {
            Vec::new()
        } else {
            special[PAGE_HIGH_KEY_OFF..PAGE_HIGH_KEY_OFF + high_key_len].to_vec()
        };
        Ok(PageHeader {
            kind,
            level,
            index_id,
            left,
            right,
            high_key,
        })
    }

    pub(super) fn write_page_header(page: &mut Page, header: &PageHeader) -> Result<()> {
        let special = page.special_bytes_mut()?;
        special.fill(0);
        crate::format::bytes::write_u32(special, 0, INDEX_MAGIC)?;
        crate::format::bytes::write_u16(special, 4, INDEX_VERSION)?;
        special[6] = header.kind;
        special[7] = 0;
        write_u16(special, PAGE_LEVEL_OFF, header.level)?;
        write_u64(special, PAGE_INDEX_ID_OFF, header.index_id.0)?;
        write_u64(
            special,
            PAGE_LEFT_OFF,
            header.left.map(|p| p.0).unwrap_or(u64::MAX),
        )?;
        write_u64(
            special,
            PAGE_RIGHT_OFF,
            header.right.map(|p| p.0).unwrap_or(u64::MAX),
        )?;
        write_u16(special, PAGE_HIGH_KEY_LEN_OFF, header.high_key.len() as u16)?;
        if !header.high_key.is_empty() {
            crate::format::bytes::write_bytes(special, PAGE_HIGH_KEY_OFF, &header.high_key)?;
        }
        Ok(())
    }

    pub(super) fn read_meta(page: &Page) -> Result<MetaHeader> {
        let special = page.special_bytes()?;
        if read_u16(special, 4)? != INDEX_VERSION {
            return Err(Error::UnsupportedVersion(read_u16(special, 4)?));
        }
        let index_id = IndexId(read_u64(special, 8)?);
        let root_page_id = PageId(read_u64(special, META_ROOT_PAGE_OFF)?);
        let root_level = read_u16(special, META_ROOT_LEVEL_OFF)?;
        let uniqueness = match special[META_UNIQUENESS_OFF] {
            0 => IndexUniqueness::NonUnique,
            1 => IndexUniqueness::Unique,
            _ => return Err(Error::CorruptPage("unknown uniqueness")),
        };
        Ok(MetaHeader {
            index_id,
            root_page_id,
            root_level,
            uniqueness,
        })
    }

    pub(super) fn write_meta(page: &mut Page, meta: &MetaHeader) -> Result<()> {
        let special = page.special_bytes_mut()?;
        special.fill(0);
        write_u32(special, 0, INDEX_MAGIC)?;
        write_u16(special, 4, INDEX_VERSION)?;
        special[6] = PAGE_META_KIND;
        special[7] = 0;
        write_u64(special, 8, meta.index_id.0)?;
        write_u64(special, META_ROOT_PAGE_OFF, meta.root_page_id.0)?;
        write_u16(special, META_ROOT_LEVEL_OFF, meta.root_level)?;
        special[META_UNIQUENESS_OFF] = meta.uniqueness as u8;
        write_u16(special, META_HIGH_KEY_LEN_OFF, 0)?;
        Ok(())
    }
}

fn decode_opt_page_id(raw: u64) -> Option<PageId> {
    if raw == u64::MAX {
        None
    } else {
        Some(PageId(raw))
    }
}

impl IndexDescriptor {
    pub fn new(index_id: IndexId, rel_id: RelId, uniqueness: IndexUniqueness) -> Self {
        Self {
            index_id,
            rel_id,
            uniqueness,
        }
    }
}

impl IndexRowRef {
    pub fn new(tuple: TuplePtr) -> Self {
        Self {
            row_id: crate::format::RowId::ZERO,
            tuple,
        }
    }

    pub fn with_row_id(row_id: crate::format::RowId, tuple: TuplePtr) -> Self {
        Self { row_id, tuple }
    }
}

pub fn compare_physical_key(left: &[u8], right: &[u8]) -> Ordering {
    left.cmp(right)
}

pub fn encode_physical_key(logical_key: &[u8], row: IndexRowRef) -> KeyBuf {
    let mut key = KeyBuf::new();
    key.extend_logical(logical_key);
    key.append_row_ref_suffix(row);
    key
}
