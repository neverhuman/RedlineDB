use redlinedb_kernel::Error;
use redlinedb_kernel::format::{DEFAULT_PAGE_SIZE, Page, PageId, PageKind, RelId};

#[test]
fn page_round_trips_header_and_cells() {
    let mut page = Page::new(DEFAULT_PAGE_SIZE, PageKind::Heap, PageId(42), RelId(7)).unwrap();

    let a = page.insert_cell(b"alpha").unwrap();
    let b = page.insert_cell(b"bravo").unwrap();

    assert_eq!(a, 0);
    assert_eq!(b, 1);
    assert_eq!(page.cell(a).unwrap(), b"alpha");
    assert_eq!(page.cell(b).unwrap(), b"bravo");

    let bytes = page.as_bytes().to_vec();
    let decoded = Page::from_bytes(bytes).unwrap();
    let header = decoded.header().unwrap();
    assert_eq!(header.kind, PageKind::Heap);
    assert_eq!(header.page_id, PageId(42));
    assert_eq!(header.rel_id, RelId(7));
    assert_eq!(header.generation.0, 1);
    assert_eq!(
        header.state as u8,
        redlinedb_kernel::format::PageState::Active as u8
    );
    assert_eq!(decoded.cell(0).unwrap(), b"alpha");
    assert_eq!(decoded.cell(1).unwrap(), b"bravo");
}

#[test]
fn page_detects_checksum_corruption() {
    let mut page = Page::new(DEFAULT_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    page.insert_cell(b"alpha").unwrap();
    page.as_mut_bytes_for_io_test()[128] ^= 0x80;

    let err = Page::from_bytes(page.as_bytes().to_vec()).unwrap_err();
    assert_eq!(err, Error::InvalidChecksum);
}

#[test]
fn overwrite_cell_rewrites_payload_and_preserves_slots() {
    let mut page = Page::new(DEFAULT_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    let a = page.insert_cell(b"alpha").unwrap();
    let b = page.insert_cell(b"bravo").unwrap();

    page.overwrite_cell(a, b"omega").unwrap();

    assert_eq!(page.cell(a).unwrap(), b"omega");
    assert_eq!(page.cell(b).unwrap(), b"bravo");
    assert_eq!(page.slot_count().unwrap(), 2);

    let decoded = Page::from_bytes(page.as_bytes().to_vec()).unwrap();
    assert_eq!(decoded.cell(a).unwrap(), b"omega");
    assert_eq!(decoded.cell(b).unwrap(), b"bravo");
}

#[test]
fn page_reinitialize_bumps_generation_and_clears_slots() {
    let mut page = Page::new(DEFAULT_PAGE_SIZE, PageKind::Heap, PageId(3), RelId(9)).unwrap();
    page.insert_cell(b"alpha").unwrap();

    page.reinitialize(
        PageKind::Heap,
        PageId(3),
        RelId(9),
        redlinedb_kernel::format::PageGeneration(2),
    )
    .unwrap();

    let header = page.header().unwrap();
    assert_eq!(header.generation.0, 2);
    assert_eq!(
        header.state as u8,
        redlinedb_kernel::format::PageState::Active as u8
    );
    assert_eq!(page.slot_count().unwrap(), 0);
}

#[test]
fn overwrite_cell_rejects_length_changes() {
    let mut page = Page::new(DEFAULT_PAGE_SIZE, PageKind::Heap, PageId(1), RelId(1)).unwrap();
    let slot = page.insert_cell(b"alpha").unwrap();

    let err = page.overwrite_cell(slot, b"too long").unwrap_err();

    assert_eq!(err, Error::CorruptPage("overwrite cell length mismatch"));
    assert_eq!(page.cell(slot).unwrap(), b"alpha");
}
