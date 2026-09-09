use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use crossbeam_utils::CachePadded;

use crate::format::{Lsn, Page, PageId, PageKind, RelId};
use crate::storage::PageFile;
use crate::storage::numa;
use crate::storage::policy::{ActiveBufferPolicy, BufferPolicy};
use crate::telemetry::Phase11Counters;
use crate::{Error, Result};

pub const DEFAULT_CHECKPOINT_BATCH_PAGES: usize = 64;

/// Minimum capacity of the prefetch worker queue. The queue size
/// otherwise scales with the pool (`capacity / 4`), but a tiny pool
/// would otherwise yield a single-slot queue that drops nearly every
/// hint.
const PREFETCH_QUEUE_MIN: usize = 32;
/// How long the worker parks when the queue drains. Park-timeout is
/// woken eagerly on every `try_prefetch` push, so the timeout only
/// matters for shutdown latency and for the (rare) case where an
/// unpark notification is lost.
const PREFETCH_PARK: Duration = Duration::from_micros(200);

/// Buffer pool with a background prefetch worker.
///
/// The previous design ran the prefetch cold load synchronously on the
/// caller's thread, which blocked SQL workers on disk I/O. WS-C4 moves
/// the cold load to a dedicated worker thread fed by a bounded
/// `ArrayQueue`. `try_prefetch` (and `prefetch`) push onto the queue
/// and return immediately; if the queue is full the hint is dropped
/// and `Phase11Counters::prefetch_dropped` is bumped.
///
/// # Drop ordering invariant
///
/// The worker thread MUST NOT hold a strong `Arc<BufferPool>` (or any
/// strong `Arc` into the same cycle), otherwise `Drop for BufferPool`
/// would never run. All pool state lives behind `Arc<Inner>` and the
/// worker upgrades a `Weak<Inner>` per pop. When the last external
/// strong ref drops, `Drop` flips the shutdown flag, unparks the
/// worker, and joins — the worker's next `weak.upgrade()` returns
/// `None` and the loop exits.
#[derive(Debug)]
pub struct BufferPool {
    inner: Arc<Inner>,
    prefetch_queue: Arc<ArrayQueue<PageId>>,
    shutdown: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug)]
struct Inner {
    page_file: Arc<PageFile>,
    capacity: usize,
    shards: Vec<Mutex<HashMap<PageId, Arc<FrameEntry>>>>,
    next_page_id: AtomicU64,
    // Phase 5 WS-B5: avoid false-sharing with adjacent counters.
    resident: CachePadded<AtomicUsize>,
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

// Phase 5 WS-B5: avoid false-sharing with adjacent counters.
#[derive(Debug, Default)]
struct BufferPoolStatsInner {
    reads: CachePadded<AtomicU64>,
    writes: CachePadded<AtomicU64>,
    evictions: CachePadded<AtomicU64>,
    checkpoint_flushes: CachePadded<AtomicU64>,
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
        // A26: hit the process-wide cached available_parallelism() so the
        // BufferPool constructor doesn't re-walk the cgroup hierarchy.
        let parallelism = crate::cached_available_parallelism();
        Self::new_with_parallelism(page_file, capacity, parallelism)
    }

    /// Like `new` but uses a caller-supplied parallelism hint instead of
    /// querying `cached_available_parallelism()`.  Use this for volatile
    /// (in-memory) databases to avoid the cgroup walk on every fresh process.
    pub(crate) fn new_with_parallelism(
        page_file: Arc<PageFile>,
        capacity: usize,
        parallelism: usize,
    ) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::CorruptPage("buffer pool capacity must be nonzero"));
        }
        let base_shard_count = capacity.min((parallelism * 4).max(16)).max(1);
        // Phase 5 WS-B6: with `--features numa` round the shard count up
        // to a multiple of the host's NUMA node count so each node owns
        // a disjoint slab of shards (`shard_idx % nodes == node_id`).
        // Without the feature `numa_node_count()` returns 1 and the
        // round-up is a no-op, so the off-feature build keeps the
        // pre-B6 shard layout byte-identical.
        let nodes = numa::numa_node_count().max(1);
        let shard_count = base_shard_count
            .div_ceil(nodes)
            .saturating_mul(nodes)
            .max(1);
        let mut shards = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            shards.push(Mutex::new(HashMap::new()));
        }
        let next_page_id = page_file.page_count()?.saturating_add(1);
        let inner = Arc::new(Inner {
            page_file,
            capacity,
            shards,
            next_page_id: AtomicU64::new(next_page_id),
            resident: CachePadded::new(AtomicUsize::new(0)),
            clock_hand: AtomicUsize::new(0),
            eviction: Mutex::new(()),
            stats: BufferPoolStatsInner::default(),
        });

        let queue_capacity = (capacity / 4).max(PREFETCH_QUEUE_MIN);
        let prefetch_queue = Arc::new(ArrayQueue::<PageId>::new(queue_capacity));
        let shutdown = Arc::new(AtomicBool::new(false));

        // Phase 5 hot-fix: prefetch worker is LAZY — spawned on first
        // try_prefetch call instead of at construction. Workloads that
        // never prefetch (in-memory DBs, short-lived CLI scripts) skip
        // the thread-spawn cost entirely (was ~0.2 ms per Database::new
        // × 1127 parity processes = ~225 ms aggregate noise).
        Ok(Self {
            inner,
            prefetch_queue,
            shutdown,
            worker: Mutex::new(None),
        })
    }

    /// Ensure the prefetch worker is spawned. No-op if already spawned.
    fn ensure_prefetch_worker(&self) {
        let mut guard = match self.worker.lock() {
            Ok(g) => g,
            Err(_) => return, // poisoned — skip; worker is best-effort
        };
        if guard.is_some() {
            return;
        }
        let weak_inner: Weak<Inner> = Arc::downgrade(&self.inner);
        let worker_queue = Arc::clone(&self.prefetch_queue);
        let worker_shutdown = Arc::clone(&self.shutdown);
        if let Ok(handle) = thread::Builder::new()
            .name("redlinedb-prefetch".to_string())
            .spawn(move || prefetch_worker(weak_inner, worker_queue, worker_shutdown))
        {
            *guard = Some(handle);
        }
        // On spawn failure we silently leave the worker unset; future
        // try_prefetch calls will simply enqueue with no consumer — the
        // queue overflows and prefetch_dropped fires. Acceptable since
        // prefetch is advisory.
    }

    pub fn allocate(&self, kind: PageKind, rel_id: RelId) -> Result<PageGuard> {
        self.inner.allocate(kind, rel_id)
    }

    pub(crate) fn page_size(&self) -> usize {
        self.inner.page_file.page_size()
    }

    pub fn pin(&self, page_id: PageId) -> Result<PageGuard> {
        self.inner.pin(page_id)
    }

    /// Push `page_id` onto the prefetch worker queue. Returns
    /// immediately. If the queue is full the hint is dropped and
    /// `Phase11Counters::prefetch_dropped` is bumped. The worker is
    /// unparked on a successful push so latency is bounded by one
    /// thread wake-up, not by the park timeout.
    pub fn try_prefetch(&self, page_id: PageId, counters: &Phase11Counters) {
        // Lazy worker: spawn on first prefetch hint. Subsequent calls
        // hit the fast path (already-Some lock check + unpark).
        self.ensure_prefetch_worker();
        match self.prefetch_queue.push(page_id) {
            Ok(()) => {
                if let Ok(guard) = self.worker.lock()
                    && let Some(handle) = guard.as_ref()
                {
                    handle.thread().unpark();
                }
            }
            Err(_) => {
                counters.prefetch_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Phase 11 W1-B advisory prefetch hint.
    ///
    /// Resident-hit fast path stays synchronous (a try_lock probe).
    /// Cold cases enqueue onto the worker queue via [`Self::try_prefetch`]
    /// rather than blocking the caller on disk I/O. Counter semantics
    /// for `prefetch_hits`/`prefetch_misses` are unchanged; the new
    /// `prefetch_dropped` counter fires only on queue overflow.
    pub fn prefetch(&self, page_id: PageId, counters: &Phase11Counters) {
        let shard_idx = self.inner.shard_idx(page_id);
        let resident = match self.inner.shards[shard_idx].try_lock() {
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
        counters.prefetch_misses.fetch_add(1, Ordering::Relaxed);
        if !ActiveBufferPolicy::prefetch_cold_load(self.inner.resident_pages(), self.inner.capacity)
        {
            return;
        }
        // Hand the cold load off to the worker; never block the
        // caller on disk I/O here.
        self.try_prefetch(page_id, counters);
    }

    pub fn flush_page(&self, page_id: PageId, durable_lsn: Lsn) -> Result<()> {
        self.inner.flush_page(page_id, durable_lsn)
    }

    pub fn flush_all(&self, durable_lsn: Lsn) -> Result<()> {
        self.inner.flush_all(durable_lsn)
    }

    pub fn write_page_direct(&self, page: &Page) -> Result<()> {
        self.inner.write_page_direct(page)
    }

    pub fn flush_dirty_batch(&self, durable_lsn: Lsn, max_pages: usize) -> Result<FlushStats> {
        self.inner.flush_dirty_batch(durable_lsn, max_pages)
    }

    pub fn flush_dirty_batches(&self, durable_lsn: Lsn, batch_pages: usize) -> Result<FlushStats> {
        self.inner.flush_dirty_batches(durable_lsn, batch_pages)
    }

    pub fn resident_pages(&self) -> usize {
        self.inner.resident_pages()
    }

    pub fn stats(&self) -> BufferPoolStats {
        self.inner.stats()
    }

    pub fn page_count(&self) -> Result<u64> {
        self.inner.page_count()
    }

    /// Lane INT: raw-bytes read used by the integrity checker to recompute
    /// CRC32 numbers when [`pin`] returns [`Error::InvalidChecksum`]. The
    /// regular `pin` path runs `Page::from_bytes`, which validates the
    /// checksum and refuses to surface bytes for a corrupt page.
    pub fn read_page_bytes_unchecked(&self, page_id: PageId) -> Result<Vec<u8>> {
        self.inner.read_page_bytes_unchecked(page_id)
    }
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let handle = self.worker.lock().ok().and_then(|mut guard| guard.take());
        if let Some(handle) = handle {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

fn prefetch_worker(
    weak_inner: Weak<Inner>,
    queue: Arc<ArrayQueue<PageId>>,
    shutdown: Arc<AtomicBool>,
) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        // Drain whatever is queued in a tight loop; only park when
        // the queue is empty. Re-check shutdown each iteration so a
        // shutdown raised while we were draining is honoured before
        // the next park.
        let mut drained_any = false;
        while let Some(page_id) = queue.pop() {
            drained_any = true;
            let Some(inner) = weak_inner.upgrade() else {
                return;
            };
            // pin() errors are swallowed by design — prefetch is
            // advisory and a failed cold load just leaves the page
            // not-warmed.
            let _ = inner.pin(page_id);
            drop(inner);
            if shutdown.load(Ordering::Acquire) {
                return;
            }
        }
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        if !drained_any {
            // Park with a small timeout so a missed unpark eventually
            // wakes us up (defensive — the producer always unparks).
            thread::park_timeout(PREFETCH_PARK);
        }
    }
}

impl Inner {
    fn allocate(&self, kind: PageKind, rel_id: RelId) -> Result<PageGuard> {
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

    fn pin(&self, page_id: PageId) -> Result<PageGuard> {
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

    fn flush_page(&self, page_id: PageId, durable_lsn: Lsn) -> Result<()> {
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

    fn flush_all(&self, durable_lsn: Lsn) -> Result<()> {
        for (_, frame) in self.all_frames()? {
            self.flush_frame(&frame, durable_lsn)?;
        }
        self.page_file.sync_data()
    }

    fn write_page_direct(&self, page: &Page) -> Result<()> {
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

    fn flush_dirty_batch(&self, durable_lsn: Lsn, max_pages: usize) -> Result<FlushStats> {
        let stats = self.flush_dirty_batch_inner(durable_lsn, max_pages)?;
        if stats.flushed_pages > 0 {
            self.page_file.sync_data()?;
        }
        Ok(stats)
    }

    fn flush_dirty_batches(&self, durable_lsn: Lsn, batch_pages: usize) -> Result<FlushStats> {
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

    fn resident_pages(&self) -> usize {
        self.resident.load(Ordering::Relaxed)
    }

    fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            resident_pages: self.resident_pages(),
            reads: self.stats.reads.load(Ordering::Relaxed),
            writes: self.stats.writes.load(Ordering::Relaxed),
            evictions: self.stats.evictions.load(Ordering::Relaxed),
            checkpoint_flushes: self.stats.checkpoint_flushes.load(Ordering::Relaxed),
        }
    }

    fn page_count(&self) -> Result<u64> {
        self.page_file.page_count()
    }

    fn read_page_bytes_unchecked(&self, page_id: PageId) -> Result<Vec<u8>> {
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
