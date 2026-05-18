use std::collections::HashMap;

use crate::format::{
    DEFAULT_PAGE_SIZE, Page, PageId, PageKind, RelId, RowId, TuplePtr, TupleVersion, TxId, UndoPtr,
};
use crate::txn::{Snapshot, TupleVisibility, TxStatusTable, UndoKind, UndoRecord};
use crate::{Error, Result};

#[derive(Debug)]
pub struct MemHeap {
    rel_id: RelId,
    pages: Vec<Page>,
    row_dir: HashMap<RowId, TuplePtr>,
    undo: Vec<UndoRecord>,
    next_row_id: u64,
    page_size: usize,
}

impl MemHeap {
    pub fn new(rel_id: RelId) -> Result<Self> {
        Ok(Self {
            rel_id,
            pages: vec![Page::new(
                DEFAULT_PAGE_SIZE,
                PageKind::Heap,
                PageId(1),
                rel_id,
            )?],
            row_dir: HashMap::new(),
            undo: Vec::new(),
            next_row_id: 1,
            page_size: DEFAULT_PAGE_SIZE,
        })
    }

    pub fn insert(&mut self, tx_id: TxId, payload: Vec<u8>) -> Result<RowId> {
        let row_id = RowId(self.next_row_id);
        self.next_row_id += 1;
        let tuple = TupleVersion::new(row_id, self.rel_id, tx_id, payload);
        let ptr = self.append_tuple(&tuple)?;
        self.row_dir.insert(row_id, ptr);
        Ok(row_id)
    }

    pub fn update(&mut self, tx_id: TxId, row_id: RowId, payload: Vec<u8>) -> Result<()> {
        let current = self.current_tuple(row_id)?;
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(UndoRecord {
            kind: UndoKind::UpdateBeforeImage,
            tx_id,
            row_id,
            prev_undo: current.undo_head,
            before_image: before.encode()?,
        });

        let mut next = TupleVersion::new(row_id, self.rel_id, tx_id, payload);
        next.undo_head = undo_ptr;
        let ptr = self.append_tuple(&next)?;
        self.row_dir.insert(row_id, ptr);
        Ok(())
    }

    pub fn delete(&mut self, tx_id: TxId, row_id: RowId) -> Result<()> {
        let current = self.current_tuple(row_id)?;
        let mut before = current.clone();
        before.end_tx = tx_id;
        let undo_ptr = self.append_undo(UndoRecord {
            kind: UndoKind::DeleteBeforeImage,
            tx_id,
            row_id,
            prev_undo: current.undo_head,
            before_image: before.encode()?,
        });

        let mut tombstone = TupleVersion::deleted(row_id, self.rel_id, tx_id);
        tombstone.undo_head = undo_ptr;
        let ptr = self.append_tuple(&tombstone)?;
        self.row_dir.insert(row_id, ptr);
        Ok(())
    }

    pub fn get(
        &self,
        tx_status: &TxStatusTable,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        row_id: RowId,
    ) -> Result<Option<Vec<u8>>> {
        let Some(ptr) = self.row_dir.get(&row_id).copied() else {
            return Ok(None);
        };

        let current = self.read_tuple(ptr)?;
        match current.visibility(tx_status, snapshot, owner) {
            TupleVisibility::Visible => Ok(Some(current.payload)),
            TupleVisibility::Deleted => Ok(None),
            TupleVisibility::Invisible => {
                self.get_from_undo(tx_status, snapshot, owner, current.undo_head)
            }
        }
    }

    pub fn row_head(&self, row_id: RowId) -> Option<TuplePtr> {
        self.row_dir.get(&row_id).copied()
    }

    fn get_from_undo(
        &self,
        tx_status: &TxStatusTable,
        snapshot: &Snapshot,
        owner: Option<TxId>,
        undo_ptr: UndoPtr,
    ) -> Result<Option<Vec<u8>>> {
        let mut cursor = undo_ptr;
        while cursor != UndoPtr::ZERO {
            let undo = self.read_undo(cursor)?;
            let tuple = TupleVersion::decode(&undo.before_image)?;
            match tuple.visibility(tx_status, snapshot, owner) {
                TupleVisibility::Visible => return Ok(Some(tuple.payload)),
                TupleVisibility::Deleted => return Ok(None),
                TupleVisibility::Invisible => cursor = undo.prev_undo,
            }
        }
        Ok(None)
    }

    fn current_tuple(&self, row_id: RowId) -> Result<TupleVersion> {
        let ptr = self
            .row_dir
            .get(&row_id)
            .copied()
            .ok_or(Error::CorruptPage("row id missing from row directory"))?;
        self.read_tuple(ptr)
    }

    fn append_tuple(&mut self, tuple: &TupleVersion) -> Result<TuplePtr> {
        let encoded = tuple.encode()?;
        let mut page_index = self.pages.len() - 1;
        let slot = match self.pages[page_index].insert_cell(&encoded) {
            Ok(slot) => slot,
            Err(Error::PageFull) => {
                let page_id = PageId((self.pages.len() + 1) as u64);
                self.pages.push(Page::new(
                    self.page_size,
                    PageKind::Heap,
                    page_id,
                    self.rel_id,
                )?);
                page_index += 1;
                self.pages[page_index].insert_cell(&encoded)?
            }
            Err(err) => return Err(err),
        };
        Ok(TuplePtr::new(PageId((page_index + 1) as u64), slot))
    }

    fn read_tuple(&self, ptr: TuplePtr) -> Result<TupleVersion> {
        if ptr.is_null() || ptr.page_id.0 == 0 {
            return Err(Error::CorruptPage("null tuple pointer"));
        }
        let page_index = ptr.page_id.0 as usize - 1;
        let page = self
            .pages
            .get(page_index)
            .ok_or(Error::CorruptPage("tuple page missing"))?;
        TupleVersion::decode(page.cell(ptr.slot)?)
    }

    fn append_undo(&mut self, record: UndoRecord) -> UndoPtr {
        self.undo.push(record);
        UndoPtr(self.undo.len() as u64)
    }

    fn read_undo(&self, ptr: UndoPtr) -> Result<&UndoRecord> {
        if ptr == UndoPtr::ZERO {
            return Err(Error::CorruptPage("null undo pointer"));
        }
        self.undo
            .get(ptr.0 as usize - 1)
            .ok_or(Error::CorruptPage("undo pointer out of bounds"))
    }
}
