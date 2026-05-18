use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::format::PageId;

#[derive(Debug)]
pub(in crate::index) struct PageLatchTable {
    shards: Vec<Mutex<HashMap<PageId, Arc<PageLatch>>>>,
}

#[derive(Debug, Default)]
pub(in crate::index) struct PageLatch {
    rw: RwLock<()>,
}

impl PageLatchTable {
    pub(super) fn new(shards: usize) -> Self {
        let shards = shards.max(1);
        let mut out = Vec::with_capacity(shards);
        for _ in 0..shards {
            out.push(Mutex::new(HashMap::new()));
        }
        Self { shards: out }
    }

    pub(super) fn get(&self, page_id: PageId) -> Arc<PageLatch> {
        let shard = self.shard(page_id);
        let mut map = self.shards[shard].lock();
        Arc::clone(
            map.entry(page_id)
                .or_insert_with(|| Arc::new(PageLatch::default())),
        )
    }

    fn shard(&self, page_id: PageId) -> usize {
        page_id.0 as usize % self.shards.len()
    }
}

impl PageLatch {
    pub(super) fn read(&self) -> parking_lot::RwLockReadGuard<'_, ()> {
        self.rw.read()
    }

    pub(super) fn write(&self) -> parking_lot::RwLockWriteGuard<'_, ()> {
        self.rw.write()
    }
}
