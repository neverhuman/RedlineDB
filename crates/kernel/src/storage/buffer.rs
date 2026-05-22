use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use crate::format::{Lsn, Page, PageId, PageKind, RelId};
use crate::storage::PageFile;
use crate::storage::policy::{ActiveBufferPolicy, BufferPolicy};
use crate::telemetry::Phase11Counters;
use crate::{Error, Result};

pub const DEFAULT_CHECKPOINT_BATCH_PAGES: usize = 64;

#[derive(Debug)]
pub struct BufferPool {
    page_file: Arc<PageFile>,
    capacity: usize,
    shards: Vec<Mutex<HashMap<PageId, Arc<FrameEntry>>>>,
    next_page_id: AtomicU64,
    resident: AtomicUsize,
    clock_hand: AtomicUsize,
    eviction: Mutex<()>,
    stats: BufferPoolStatsInner,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferPoolStats {
    pub resident_pages: usize,
    pub reads: u64,
    pub writes: u64,
    pub evictions: u64,
    pub checkpoint_flushes: u64,
}

#[derive(Debug, Default)]
struct BufferPoolStatsInner {
    reads: AtomicU64,
    writes: AtomicU64,
    evictions: AtomicU64,
    checkpoint_flushes: AtomicU64,
}

#[derive(Debug)]
struct FrameEntry {
    state: Mutex<FrameState>,
    ready: Condvar,
}

#[derive(Debug)]
pub(crate) struct FrameState {
    pub(crate) page: Option<Page>,
    pin_count: usize,
    pub(crate) dirty: bool,
    usage_count: u8,
    write_in_progress: bool,
    load_failed: bool,
}

#[derive(Debug)]
pub struct PageGuard {
    page_id: PageId,
    frame: Arc<FrameEntry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlushStats {
    pub flushed_pages: usize,
    pub batches: usize,
}

impl BufferPool {
    pub fn new(page_file: Arc<PageFile>, capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::CorruptPage("buffer pool capacity must be nonzero"));
        }
        let parallelism = thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(4);
        let shard_count = capacity.min((parallelism * 4).max(16)).max(1);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Mutex::new(HashMap::new()));
        }
        let next_page_id = page_file.page_count()?.saturating_add(1);
        Ok(Self {
            page_file,
            capacity,
            shards,
            next_page_id: AtomicU64::new(next_page_id),
            resident: AtomicUsize::new(0),
            clock_hand: AtomicUsize::new(0),
            eviction: Mutex::new(()),
            stats: BufferPoolStatsInner::default(),
        })
    }

    pub fn allocate(&self, kind: PageKind, rel_id: RelId) -> Result<PageGuard> {
        self.ensure_capacity(Lsn::ZERO)?;
        let page_id = PageId(self.next_page_id.fetch_add(1, Ordering::Relaxed));
        let page = Page::new(self.page_file.page_size(), kind, page_id, rel_id)?;
        let frame = Arc::new(FrameEntry {
            state: Mutex::new(FrameState {
                page: Some(page),
                pin_count: 1,
                dirty: true,
                usage_count: 1,
                write_in_progress: false,
                load_failed: false,
            }),
            ready: Condvar::new(),
        });
        self.insert_new_frame(page_id, Arc::clone(&frame))?;
        Ok(PageGuard { page_id, frame })
    }

    pub fn pin(&self, page_id: PageId) -> Result<PageGuard> {
        loop {
            if let Some(frame) = self.lookup_frame(page_id)? {
                let mut state = frame
                    .state
                    .lock()
                    .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
                while state.page.is_none() && !state.load_failed {
                    state = frame
                        .ready
                        .wait(state)
                        .map_err(|_| Error::CorruptPage("buffer frame wait poisoned"))?;
                }
                if state.load_failed {
                    drop(state);
                    continue;
                }
                state.pin_count += 1;
                state.usage_count = state.usage_count.saturating_add(1).min(5);
                drop(state);
                return Ok(PageGuard { page_id, frame });
            }

            self.ensure_capacity(Lsn::ZERO)?;
            let frame = Arc::new(FrameEntry {
                state: Mutex::new(FrameState {
                    page: None,
                    pin_count: 1,
                    dirty: false,
                    usage_count: 1,
                    write_in_progress: false,
                    load_failed: false,
                }),
                ready: Condvar::new(),
            });

            if self.try_insert_loading_frame(page_id, Arc::clone(&frame))? {
                let page = match self.page_file.read_page(page_id) {
                    Ok(page) => page,
                    Err(err) => {
                        self.remove_loading_frame(page_id, &frame)?;
                        return Err(err);
                    }
                };
                self.stats.reads.fetch_add(1, Ordering::Relaxed);
                let mut state = frame
                    .state
                    .lock()
                    .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
                state.page = Some(page);
                frame.ready.notify_all();
                drop(state);
                return Ok(PageGuard { page_id, frame });
            }
        }
    }

    /// Phase 11 W1-B advisory prefetch hint.
    ///
    /// Best-effort attempt to warm `page_id` in the buffer pool. The
    /// call is purely advisory and follows three invariants:
    ///
    /// * **Never propagates errors** — a synchronous cold-load is
    ///   attempted via [`Self::pin`] when the probe says the page is
    ///   not resident, but any pin error (I/O failure, eviction race,
    ///   etc.) is swallowed. The caller never observes a `Result`.
    /// * **Never waits for a contended residency probe** — the shard
    ///   check uses [`Mutex::try_lock`]. If the shard is contended we
    ///   drop the hint silently and bump `prefetch_misses`.
    /// * **Never depends on the warm side effect** — a cold load may
    ///   block briefly on disk I/O, but the caller must remain correct
    ///   if the page is not warmed.
    ///
    /// Counter semantics (matches the W1-B design note: "miss" means
    /// the page was *not* resident at probe time, regardless of
    /// whether the subsequent load actually proceeded — this is the
    /// metric W1-B verification cares about):
    ///
    /// * `prefetch_hits` — page was already resident at probe time.
    /// * `prefetch_misses` — page was not resident, *or* the shard
    ///   was contended (hint dropped). The cold-load attempt is
    ///   best-effort; the miss counter is bumped regardless.
    pub fn prefetch(&self, page_id: PageId, counters: &Phase11Counters) {
        let shard_idx = self.shard_idx(page_id);
        let resident = match self.shards[shard_idx].try_lock() {
            Ok(shard) => shard.contains_key(&page_id),
            Err(_) => {
                // Contended shard: drop the hint as a miss without
                // attempting a cold load.
                counters.prefetch_misses.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        if resident {
            counters.prefetch_hits.fetch_add(1, Ordering::Relaxed);
            return;
        }
        // Page was not resident at probe time — count it as a miss
        // up-front, then attempt a synchronous cold load. The pin
        // path may briefly block on disk I/O; a future iteration
        // can defer this to an existing thread pool. Errors are
        // swallowed by design — prefetch is purely advisory.
        counters.prefetch_misses.fetch_add(1, Ordering::Relaxed);
        if !ActiveBufferPolicy::prefetch_cold_load(self.resident_pages(), self.capacity) {
            return;
        }
        if let Ok(_guard) = self.pin(page_id) {
            // Drop the guard immediately; the goal is just to warm
            // the buffer pool, not to keep the page pinned.
        }
    }

    pub fn flush_page(&self, page_id: PageId, durable_lsn: Lsn) -> Result<()> {
        let Some(frame) = self.lookup_frame(page_id)? else {
            return Ok(());
        };
        self.flush_frame(&frame, durable_lsn).map(|flushed| {
            if flushed {
                self.stats
                    .checkpoint_flushes
                    .fetch_add(1, Ordering::Relaxed);
            }
        })
    }

    pub fn flush_all(&self, durable_lsn: Lsn) -> Result<()> {
        for (_, frame) in self.all_frames()? {
            self.flush_frame(&frame, durable_lsn)?;
        }
        self.page_file.sync_data()
    }

    pub fn write_page_direct(&self, page: &Page) -> Result<()> {
        self.page_file.write_page(page)?;
        let page_id = page.header()?.page_id;
        let next = page_id.0.saturating_add(1);
        let mut current = self.next_page_id.load(Ordering::Relaxed);
        while current < next {
            match self.next_page_id.compare_exchange(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
        Ok(())
    }

    pub fn flush_dirty_batch(&self, durable_lsn: Lsn, max_pages: usize) -> Result<FlushStats> {
        let stats = self.flush_dirty_batch_inner(durable_lsn, max_pages)?;
        if stats.flushed_pages > 0 {
            self.page_file.sync_data()?;
        }
        Ok(stats)
    }

    pub fn flush_dirty_batches(&self, durable_lsn: Lsn, batch_pages: usize) -> Result<FlushStats> {
        let batch_pages = batch_pages.max(1);
        let mut flushed_pages = 0_usize;
        let mut batches = 0_usize;

        loop {
            let batch = self.flush_dirty_batch_inner(durable_lsn, batch_pages)?;
            if batch.flushed_pages == 0 {
                break;
            }
            flushed_pages += batch.flushed_pages;
            batches += 1;
            if batch.flushed_pages < batch_pages {
                break;
            }
            thread::yield_now();
        }

        if flushed_pages > 0 {
            self.page_file.sync_data()?;
        }

        Ok(FlushStats {
            flushed_pages,
            batches,
        })
    }

    fn flush_dirty_batch_inner(&self, durable_lsn: Lsn, max_pages: usize) -> Result<FlushStats> {
        let max_pages = max_pages.max(1);
        let frames = self.dirty_frames(durable_lsn)?;
        let policy_pages =
            ActiveBufferPolicy::dirty_batch_pages(self.resident_pages(), frames.len()).max(1);
        let page_limit = max_pages.min(policy_pages);
        let mut flushed_pages = 0_usize;
        for frame in frames.into_iter().take(page_limit) {
            if self.flush_frame_if_durable(&frame, durable_lsn)? {
                flushed_pages += 1;
                self.stats
                    .checkpoint_flushes
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(FlushStats {
            flushed_pages,
            batches: usize::from(flushed_pages > 0),
        })
    }

    pub fn resident_pages(&self) -> usize {
        self.resident.load(Ordering::Relaxed)
    }

    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            resident_pages: self.resident_pages(),
            reads: self.stats.reads.load(Ordering::Relaxed),
            writes: self.stats.writes.load(Ordering::Relaxed),
            evictions: self.stats.evictions.load(Ordering::Relaxed),
            checkpoint_flushes: self.stats.checkpoint_flushes.load(Ordering::Relaxed),
        }
    }

    pub fn page_count(&self) -> Result<u64> {
        self.page_file.page_count()
    }

    /// Lane INT: raw-bytes read used by the integrity checker to recompute
    /// CRC32 numbers when [`pin`] returns [`Error::InvalidChecksum`]. The
    /// regular `pin` path runs `Page::from_bytes`, which validates the
    /// checksum and refuses to surface bytes for a corrupt page.
    pub fn read_page_bytes_unchecked(&self, page_id: PageId) -> Result<Vec<u8>> {
        self.page_file.read_page_bytes_unchecked(page_id)
    }

    fn insert_new_frame(&self, page_id: PageId, frame: Arc<FrameEntry>) -> Result<()> {
        let shard_idx = self.shard_idx(page_id);
        let mut shard = self.shards[shard_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
        if shard.insert(page_id, frame).is_some() {
            return Err(Error::CorruptPage("allocated page already resident"));
        }
        self.resident.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn try_insert_loading_frame(&self, page_id: PageId, frame: Arc<FrameEntry>) -> Result<bool> {
        let shard_idx = self.shard_idx(page_id);
        let mut shard = self.shards[shard_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
        if shard.contains_key(&page_id) {
            return Ok(false);
        }
        shard.insert(page_id, frame);
        self.resident.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }

    fn lookup_frame(&self, page_id: PageId) -> Result<Option<Arc<FrameEntry>>> {
        let shard = self.shards[self.shard_idx(page_id)]
            .lock()
            .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
        Ok(shard.get(&page_id).cloned())
    }

    fn ensure_capacity(&self, durable_lsn: Lsn) -> Result<()> {
        if self.resident.load(Ordering::Relaxed) < self.capacity {
            return Ok(());
        }

        let _eviction = self
            .eviction
            .lock()
            .map_err(|_| Error::CorruptPage("buffer eviction mutex poisoned"))?;
        while self.resident.load(Ordering::Relaxed) >= self.capacity {
            if !self.evict_one(durable_lsn)? {
                return Err(Error::CorruptPage(
                    "no unpinned frame available for eviction",
                ));
            }
        }
        Ok(())
    }

    fn evict_one(&self, durable_lsn: Lsn) -> Result<bool> {
        let frames = self.all_frames()?;
        if frames.is_empty() {
            return Ok(false);
        }

        let start = self.clock_hand.fetch_add(1, Ordering::Relaxed);
        for idx in 0..frames.len().saturating_mul(2) {
            let (page_id, frame) = &frames[(start + idx) % frames.len()];
            let mut state = frame
                .state
                .lock()
                .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
            if state.pin_count > 0 || state.write_in_progress || state.page.is_none() {
                continue;
            }
            if state.usage_count > 0 {
                state.usage_count -= 1;
                continue;
            }
            if state.dirty {
                let page_lsn = state
                    .page
                    .as_ref()
                    .ok_or(Error::CorruptPage("resident frame missing page"))?
                    .header()?
                    .page_lsn;
                if page_lsn > durable_lsn {
                    continue;
                }
                drop(state);
                self.flush_frame_if_durable(frame, durable_lsn)?;
                state = frame
                    .state
                    .lock()
                    .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
                if state.pin_count > 0 || state.dirty || state.write_in_progress {
                    continue;
                }
            }
            drop(state);
            if self.remove_frame_if_unpinned(*page_id, frame)? {
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn remove_loading_frame(&self, page_id: PageId, frame: &Arc<FrameEntry>) -> Result<()> {
        let shard_idx = self.shard_idx(page_id);
        let mut shard = self.shards[shard_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
        if shard
            .get(&page_id)
            .map(|resident| Arc::ptr_eq(resident, frame))
            .unwrap_or(false)
        {
            if let Ok(mut state) = frame.state.lock() {
                state.load_failed = true;
            }
            shard.remove(&page_id);
            self.resident.fetch_sub(1, Ordering::Relaxed);
        }
        frame.ready.notify_all();
        Ok(())
    }

    fn remove_frame_if_unpinned(&self, page_id: PageId, frame: &Arc<FrameEntry>) -> Result<bool> {
        let shard_idx = self.shard_idx(page_id);
        let mut shard = self.shards[shard_idx]
            .lock()
            .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
        if !shard
            .get(&page_id)
            .map(|resident| Arc::ptr_eq(resident, frame))
            .unwrap_or(false)
        {
            return Ok(false);
        }
        let state = frame
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
        if state.pin_count > 0 || state.dirty || state.write_in_progress || state.page.is_none() {
            return Ok(false);
        }
        drop(state);
        shard.remove(&page_id);
        self.resident.fetch_sub(1, Ordering::Relaxed);
        Ok(true)
    }

    fn dirty_frames(&self, durable_lsn: Lsn) -> Result<Vec<Arc<FrameEntry>>> {
        let mut frames = Vec::new();
        for shard in &self.shards {
            let shard = shard
                .lock()
                .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
            for frame in shard.values() {
                let state = frame
                    .state
                    .lock()
                    .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
                let Some(page) = state.page.as_ref() else {
                    continue;
                };
                if state.dirty && !state.write_in_progress && page.header()?.page_lsn <= durable_lsn
                {
                    frames.push(Arc::clone(frame));
                }
            }
        }
        Ok(frames)
    }

    fn all_frames(&self) -> Result<Vec<(PageId, Arc<FrameEntry>)>> {
        let mut frames = Vec::new();
        for shard in &self.shards {
            let shard = shard
                .lock()
                .map_err(|_| Error::CorruptPage("buffer shard poisoned"))?;
            frames.extend(
                shard
                    .iter()
                    .map(|(page_id, frame)| (*page_id, Arc::clone(frame))),
            );
        }
        Ok(frames)
    }

    fn flush_frame(&self, frame: &Arc<FrameEntry>, durable_lsn: Lsn) -> Result<bool> {
        self.flush_frame_checked(frame, durable_lsn, true)
    }

    fn flush_frame_if_durable(&self, frame: &Arc<FrameEntry>, durable_lsn: Lsn) -> Result<bool> {
        self.flush_frame_checked(frame, durable_lsn, false)
    }

    fn flush_frame_checked(
        &self,
        frame: &Arc<FrameEntry>,
        durable_lsn: Lsn,
        strict: bool,
    ) -> Result<bool> {
        let (page, written_lsn) = {
            let mut state = frame
                .state
                .lock()
                .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
            while state.page.is_none() {
                state = frame
                    .ready
                    .wait(state)
                    .map_err(|_| Error::CorruptPage("buffer frame wait poisoned"))?;
            }
            if !state.dirty {
                return Ok(false);
            }
            let page = state
                .page
                .as_ref()
                .ok_or(Error::CorruptPage("resident frame missing page"))?;
            let page_lsn = page.header()?.page_lsn;
            if page_lsn > durable_lsn {
                return if strict {
                    Err(Error::CorruptPage("dirty page lsn exceeds durable wal lsn"))
                } else {
                    Ok(false)
                };
            }
            let page = page.clone();
            state.write_in_progress = true;
            (page, page_lsn)
        };

        let write_result = self.page_file.write_page(&page);
        let mut state = frame
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("buffer frame poisoned"))?;
        state.write_in_progress = false;
        match write_result {
            Ok(()) => {
                let current_lsn = state
                    .page
                    .as_ref()
                    .ok_or(Error::CorruptPage("resident frame missing page"))?
                    .header()?
                    .page_lsn;
                if current_lsn <= written_lsn {
                    state.dirty = false;
                }
                self.stats.writes.fetch_add(1, Ordering::Relaxed);
                frame.ready.notify_all();
                Ok(true)
            }
            Err(err) => {
                frame.ready.notify_all();
                Err(err)
            }
        }
    }

    fn shard_idx(&self, page_id: PageId) -> usize {
        page_id.0 as usize % self.shards.len()
    }
}

impl PageGuard {
    pub fn page_id(&self) -> PageId {
        self.page_id
    }

    pub fn with_page<R>(&self, f: impl FnOnce(&Page) -> Result<R>) -> Result<R> {
        let frame = self.frame()?;
        let page = frame
            .page
            .as_ref()
            .ok_or(Error::CorruptPage("resident frame missing page"))?;
        f(page)
    }

    pub fn with_page_mut<R>(&self, f: impl FnOnce(&mut Page) -> Result<R>) -> Result<R> {
        let mut frame = self.mutable_frame()?;
        let page = frame
            .page
            .as_mut()
            .ok_or(Error::CorruptPage("resident frame missing page"))?;
        f(page)
    }

    pub fn mark_dirty(&self, lsn: Lsn) -> Result<()> {
        let mut frame = self.mutable_frame()?;
        let page = frame
            .page
            .as_mut()
            .ok_or(Error::CorruptPage("resident frame missing page"))?;
        page.set_page_lsn(lsn)?;
        frame.dirty = true;
        Ok(())
    }

    fn frame(&self) -> Result<MutexGuard<'_, FrameState>> {
        self.frame
            .state
            .lock()
            .map_err(|_| Error::CorruptPage("buffer frame poisoned"))
    }

    pub(crate) fn mutable_frame(&self) -> Result<MutexGuard<'_, FrameState>> {
        let mut frame = self.frame()?;
        while frame.write_in_progress {
            frame = self
                .frame
                .ready
                .wait(frame)
                .map_err(|_| Error::CorruptPage("buffer frame wait poisoned"))?;
        }
        Ok(frame)
    }
}

impl Drop for PageGuard {
    fn drop(&mut self) {
        if let Ok(mut frame) = self.frame() {
            frame.pin_count = frame.pin_count.saturating_sub(1);
            self.frame.ready.notify_all();
        }
    }
}
