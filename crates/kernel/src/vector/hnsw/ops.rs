use super::*;

pub(crate) fn append_node_to_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    record: &NodeRecord,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<u16> {
    let guard = buffer.pin(page_id)?;
    let slot = guard.with_page_mut(|page| {
        let bytes = record.encode();
        page.insert_cell(&bytes)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(slot)
}

/// Overwrite the cell at `(page_id, slot)` with `record`. Length must
/// match — neighbor-list resize during link-pruning uses
/// [`rewrite_data_page`] instead, which compacts the entire page.
pub(crate) fn overwrite_node_in_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    slot: u16,
    record: &NodeRecord,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    guard.with_page_mut(|page| {
        let bytes = record.encode();
        page.overwrite_cell(slot, &bytes)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(())
}

/// Rewrite an HNSW data page from a list of records. Used when a record's
/// encoded size grows (neighbor list extended) and an in-place
/// [`overwrite_node_in_page`] would no longer fit. Returns the leftover
/// records that didn't fit; caller pushes them into a new page.
pub(crate) fn rewrite_data_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    records: &[NodeRecord],
    next_data_page: Option<PageId>,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<usize> {
    let guard = buffer.pin(page_id)?;
    let written = guard.with_page_mut(|page| {
        let rel_id = page.header()?.rel_id;
        page.reinitialize_with_special(
            PageKind::BtreeLeaf,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )?;
        write_data_page_header(page, next_data_page)?;
        let capacity = data_page_body_capacity(page.as_bytes().len());
        let mut used = 0_usize;
        let mut written = 0_usize;
        for record in records {
            let cost = node_record_slot_cost(record);
            if used + cost > capacity {
                break;
            }
            page.insert_cell(&record.encode())?;
            used += cost;
            written += 1;
        }
        Ok::<usize, Error>(written)
    })?;
    record_page_image(buffer, page_id, wal, tx_id)?;
    Ok(written)
}

/// Read every NodeRecord cell from a data page. Used by `open()` to
/// reconstruct the in-memory graph.
pub(crate) fn read_records_from_page(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
) -> Result<Vec<NodeRecord>> {
    let guard = buffer.pin(page_id)?;
    guard.with_page(|page| {
        let count = page.slot_count()?;
        let mut out = Vec::with_capacity(count as usize);
        for slot in 0..count {
            let cell = page.cell(slot)?;
            out.push(NodeRecord::decode(cell)?);
        }
        Ok(out)
    })
}

pub(crate) fn allocate_meta_page(buffer: &Arc<BufferPool>, rel_id: RelId) -> Result<PageId> {
    let guard = buffer.allocate(PageKind::BtreeMeta, rel_id)?;
    let page_id = guard.page_id();
    guard.with_page_mut(|page| {
        page.reinitialize_with_special(
            PageKind::BtreeMeta,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )
    })?;
    guard.mark_dirty(Lsn(1))?;
    Ok(page_id)
}

pub(crate) fn allocate_data_page(buffer: &Arc<BufferPool>, rel_id: RelId) -> Result<PageId> {
    let guard = buffer.allocate(PageKind::BtreeLeaf, rel_id)?;
    let page_id = guard.page_id();
    guard.with_page_mut(|page| {
        page.reinitialize_with_special(
            PageKind::BtreeLeaf,
            page_id,
            rel_id,
            PageGeneration::ONE,
            HNSW_SPECIAL_LEN,
        )?;
        write_data_page_header(page, None)
    })?;
    guard.mark_dirty(Lsn(1))?;
    Ok(page_id)
}

pub(crate) fn flush_meta(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    snap: &MetaSnapshot,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    guard.with_page_mut(|page| write_meta(page, snap))?;
    record_page_image(buffer, page_id, wal, tx_id)
}

fn record_page_image(
    buffer: &Arc<BufferPool>,
    page_id: PageId,
    wal: Option<&Arc<WalCoordinator>>,
    tx_id: TxId,
) -> Result<()> {
    let guard = buffer.pin(page_id)?;
    if tx_id == TxId::ZERO {
        return guard.mark_dirty(Lsn(1));
    }
    let Some(wal) = wal else {
        return guard.mark_dirty(Lsn(1));
    };
    let mut page = guard.with_page(|page| Ok(page.clone()))?;
    page.set_page_lsn(Lsn::ZERO)?;
    let payload = WalPayload::PageImage {
        page_id,
        page_lsn: Lsn::ZERO,
        page_bytes: page.as_bytes().to_vec(),
    };
    let append = wal.append(WalRecordKind::PageImage, tx_id, payload.encode()?)?;
    guard.mark_dirty(append.end_lsn)
}
