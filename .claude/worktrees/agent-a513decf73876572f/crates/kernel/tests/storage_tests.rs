use redlinedb_kernel::Error;
use redlinedb_kernel::format::{Csn, Lsn, Page, PageId, PageKind, RelId, TxId};
use redlinedb_kernel::storage::{
    BufferPool, ControlFile, ControlStore, PageFile, TxStatusCheckpoint, TxStatusStore,
};
use redlinedb_kernel::wal::WalPayload;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

const TEST_PAGE_SIZE: usize = 4096;

#[test]
fn page_file_writes_reads_and_validates_page() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("data.redline");
    let file = PageFile::create(&path, TEST_PAGE_SIZE).unwrap();
    let mut page = Page::new(TEST_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(7)).unwrap();
    page.insert_cell(b"hello").unwrap();
    file.write_page(&page).unwrap();
    file.sync_data().unwrap();

    let reopened = PageFile::open(&path, TEST_PAGE_SIZE).unwrap();
    let read = reopened.read_page(PageId(1)).unwrap();
    assert_eq!(read.header().unwrap().page_id, PageId(1));
    assert_eq!(read.cell(0).unwrap(), b"hello");
}

#[test]
fn page_file_rejects_wrong_size_write() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("data.redline");
    let file = PageFile::create(&path, TEST_PAGE_SIZE).unwrap();
    let page = Page::new(2048, PageKind::Heap, PageId(1), RelId(1)).unwrap();

    let err = file.write_page(&page).unwrap_err();
    assert!(matches!(err, Error::BufferTooSmall { .. }));
}

#[test]
fn page_file_detects_checksum_corruption_on_read() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("data.redline");
    let file = PageFile::create(&path, TEST_PAGE_SIZE).unwrap();
    let page = Page::new(TEST_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    file.write_page(&page).unwrap();
    file.sync_data().unwrap();

    let mut raw = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    raw.seek(SeekFrom::Start(128)).unwrap();
    raw.write_all(&[0xff]).unwrap();
    raw.sync_data().unwrap();

    let err = file.read_page(PageId(1)).unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn buffer_pool_pins_same_page_and_persists_dirty_page_after_wal_durable() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(Arc::clone(&file), 2).unwrap();

    let guard = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let page_id = guard.page_id();
    guard
        .with_page_mut(|page| {
            page.insert_cell(b"alpha")?;
            Ok(())
        })
        .unwrap();
    guard.mark_dirty(Lsn(10)).unwrap();

    let second_pin = pool.pin(page_id).unwrap();
    second_pin
        .with_page(|page| {
            assert_eq!(page.cell(0).unwrap(), b"alpha");
            Ok(())
        })
        .unwrap();
    drop(second_pin);
    drop(guard);

    pool.flush_page(page_id, Lsn(10)).unwrap();
    let read = file.read_page(page_id).unwrap();
    assert_eq!(read.cell(0).unwrap(), b"alpha");
}

#[test]
fn buffer_pool_enforces_wal_before_data() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(file, 2).unwrap();

    let guard = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let page_id = guard.page_id();
    guard.mark_dirty(Lsn(50)).unwrap();
    drop(guard);

    let err = pool.flush_page(page_id, Lsn(49)).unwrap_err();
    assert_eq!(
        err,
        Error::CorruptPage("dirty page lsn exceeds durable wal lsn")
    );
    pool.flush_page(page_id, Lsn(50)).unwrap();
}

#[test]
fn buffer_pool_eviction_skips_pinned_pages_and_stays_bounded() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(file, 2).unwrap();

    let pinned = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let first_id = pinned.page_id();
    pinned.mark_dirty(Lsn(1)).unwrap();
    let second = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let second_id = second.page_id();
    second.mark_dirty(Lsn(1)).unwrap();
    drop(second);
    pool.flush_page(second_id, Lsn(1)).unwrap();

    let third = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    assert_eq!(pool.resident_pages(), 2);
    assert_ne!(third.page_id(), first_id);
    assert_ne!(third.page_id(), second_id);
    drop(pinned);
}

#[test]
fn buffer_pool_errors_when_all_pages_are_pinned() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(file, 1).unwrap();

    let _pinned = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let err = pool.allocate(PageKind::Heap, RelId(1)).unwrap_err();
    assert_eq!(
        err,
        Error::CorruptPage("no unpinned frame available for eviction")
    );
}

#[test]
fn buffer_pool_allows_concurrent_disjoint_page_pins() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = Arc::new(BufferPool::new(file, 16).unwrap());
    let mut ids = Vec::new();
    for _ in 0..8 {
        let guard = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
        guard.mark_dirty(Lsn(1)).unwrap();
        ids.push(guard.page_id());
    }
    pool.flush_all(Lsn(1)).unwrap();

    let handles: Vec<_> = ids
        .into_iter()
        .map(|page_id| {
            let pool = Arc::clone(&pool);
            thread::spawn(move || {
                let guard = pool.pin(page_id).unwrap();
                guard.with_page(|page| page.header()).unwrap().page_id
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.join().unwrap().0 > 0);
    }
}

#[test]
fn buffer_pool_concurrent_cold_pins_share_one_resident_frame() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("data.redline");
    let file = Arc::new(PageFile::create(&path, TEST_PAGE_SIZE).unwrap());
    let mut page = Page::new(TEST_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    page.insert_cell(b"shared").unwrap();
    file.write_page(&page).unwrap();
    file.sync_data().unwrap();

    let pool = Arc::new(BufferPool::new(file, 8).unwrap());
    let barrier = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let pool = Arc::clone(&pool);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let guard = pool.pin(PageId(1)).unwrap();
                guard
                    .with_page(|page| {
                        assert_eq!(page.cell(0).unwrap(), b"shared");
                        Ok(())
                    })
                    .unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = pool.stats();
    assert_eq!(stats.resident_pages, 1);
    assert_eq!(stats.reads, 1);
}

#[test]
fn buffer_pool_flush_dirty_batch_is_bounded_and_lsn_safe() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(Arc::clone(&file), 8).unwrap();
    let mut ids = Vec::new();
    for idx in 0..5 {
        let guard = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
        guard
            .with_page_mut(|page| {
                page.insert_cell(&[idx])?;
                Ok(())
            })
            .unwrap();
        guard
            .mark_dirty(if idx < 4 { Lsn(10) } else { Lsn(99) })
            .unwrap();
        ids.push(guard.page_id());
    }

    let first = pool.flush_dirty_batch(Lsn(10), 2).unwrap();
    assert_eq!(first.flushed_pages, 2);
    assert_eq!(first.batches, 1);
    let second = pool.flush_dirty_batch(Lsn(10), 8).unwrap();
    assert_eq!(second.flushed_pages, 2);

    let future = pool.flush_dirty_batch(Lsn(10), 8).unwrap();
    assert_eq!(future.flushed_pages, 0);
    assert!(file.read_page(ids[4]).is_err());

    let final_batch = pool.flush_dirty_batch(Lsn(99), 8).unwrap();
    assert_eq!(final_batch.flushed_pages, 1);
    assert_eq!(file.read_page(ids[4]).unwrap().cell(0).unwrap(), &[4]);
}

#[test]
fn buffer_pool_eviction_flushes_durable_dirty_victim() {
    let temp = TempDir::new().unwrap();
    let file =
        Arc::new(PageFile::create(temp.path().join("data.redline"), TEST_PAGE_SIZE).unwrap());
    let pool = BufferPool::new(Arc::clone(&file), 1).unwrap();

    let first = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    let first_id = first.page_id();
    first
        .with_page_mut(|page| {
            page.insert_cell(b"first")?;
            Ok(())
        })
        .unwrap();
    first.mark_dirty(Lsn(0)).unwrap();
    drop(first);

    let second = pool.allocate(PageKind::Heap, RelId(1)).unwrap();
    assert_ne!(second.page_id(), first_id);
    assert_eq!(pool.resident_pages(), 1);
    assert_eq!(file.read_page(first_id).unwrap().cell(0).unwrap(), b"first");
    assert_eq!(pool.stats().evictions, 1);
}

#[test]
fn page_image_payload_round_trips_and_restores_page() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("data.redline");
    let file = PageFile::create(&path, TEST_PAGE_SIZE).unwrap();
    let mut page = Page::new(TEST_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    page.insert_cell(b"image").unwrap();
    page.set_page_lsn(Lsn(77)).unwrap();

    let payload = WalPayload::PageImage {
        page_id: PageId(1),
        page_lsn: Lsn(77),
        page_bytes: page.as_bytes().to_vec(),
    };
    let decoded = WalPayload::decode(&payload.encode().unwrap()).unwrap();
    let WalPayload::PageImage {
        page_id,
        page_lsn,
        page_bytes,
    } = decoded
    else {
        panic!("expected page image");
    };
    assert_eq!(page_id, PageId(1));
    assert_eq!(page_lsn, Lsn(77));

    let restored = Page::from_bytes(page_bytes).unwrap();
    file.write_page(&restored).unwrap();
    file.sync_data().unwrap();
    let read = file.read_page(PageId(1)).unwrap();
    assert_eq!(read.header().unwrap().page_lsn, Lsn(77));
    assert_eq!(read.cell(0).unwrap(), b"image");
}

#[test]
fn page_image_payload_rejects_truncated_image() {
    let payload = WalPayload::PageImage {
        page_id: PageId(1),
        page_lsn: Lsn(1),
        page_bytes: vec![1, 2, 3, 4],
    };
    let mut encoded = payload.encode().unwrap();
    encoded.pop();

    let err = WalPayload::decode(&encoded).unwrap_err();
    assert!(matches!(err, Error::BufferTooSmall { .. }));
}

#[test]
fn control_file_round_trips_and_validates_checksum() {
    let control = ControlFile {
        generation: 9,
        checkpoint_lsn: Lsn(1234),
        page_count: 44,
    };

    let mut encoded = control.encode().unwrap();
    assert_eq!(ControlFile::decode(&encoded).unwrap(), control);

    encoded[24] ^= 0x80;
    assert_eq!(
        ControlFile::decode(&encoded).unwrap_err(),
        Error::InvalidChecksum
    );
}

#[test]
fn control_store_loads_latest_generation_and_falls_back_from_corrupt_copy() {
    let temp = TempDir::new().unwrap();
    let store = ControlStore::new(temp.path()).unwrap();

    let first = store.write_next(None, Lsn(10), 3).unwrap();
    let second = store.write_next(Some(first), Lsn(20), 7).unwrap();
    assert_eq!(store.load_latest().unwrap(), Some(second));

    let mut newer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("CONTROL_B"))
        .unwrap();
    newer.seek(SeekFrom::Start(24)).unwrap();
    newer.write_all(&[0xee]).unwrap();
    newer.sync_data().unwrap();

    assert_eq!(store.load_latest().unwrap(), Some(first));
}

#[test]
fn tx_status_checkpoint_round_trips_sorted_commits() {
    let checkpoint = TxStatusCheckpoint {
        generation: 3,
        next_tx: TxId(10),
        next_csn: Csn(91),
        published_csn: Csn(90),
        entries: vec![(TxId(9), Csn(90)), (TxId(2), Csn(20)), (TxId(5), Csn(50))],
    };

    let decoded = TxStatusCheckpoint::decode(&checkpoint.encode().unwrap()).unwrap();
    assert_eq!(decoded.generation, 3);
    assert_eq!(decoded.next_tx, TxId(10));
    assert_eq!(decoded.next_csn, Csn(91));
    assert_eq!(decoded.published_csn, Csn(90));
    assert_eq!(
        decoded.entries,
        vec![(TxId(2), Csn(20)), (TxId(5), Csn(50)), (TxId(9), Csn(90))]
    );
}

#[test]
fn tx_status_store_loads_generation_and_detects_corruption() {
    let temp = TempDir::new().unwrap();
    let store = TxStatusStore::new(temp.path()).unwrap();
    let checkpoint = TxStatusCheckpoint {
        generation: 8,
        next_tx: TxId(5),
        next_csn: Csn(3),
        published_csn: Csn(2),
        entries: vec![(TxId(1), Csn(1)), (TxId(4), Csn(2))],
    };
    store.write(&checkpoint).unwrap();
    assert_eq!(store.load(8).unwrap(), checkpoint);

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("TX_STATUS_00000000000000000008"))
        .unwrap();
    file.seek(SeekFrom::Start(40)).unwrap();
    file.write_all(&[0x99]).unwrap();
    file.sync_data().unwrap();

    assert_eq!(store.load(8).unwrap_err(), Error::InvalidChecksum);
}
