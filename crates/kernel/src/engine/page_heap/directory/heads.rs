use std::collections::HashMap;
use std::sync::RwLock;

use crate::format::{RelId, RowId, TuplePtr};
use crate::{Error, Result};

use super::super::PageBackedHeap;
use super::super::policy::{ActiveHeapPlacementPolicy, HeapPlacementPolicy};

impl PageBackedHeap {
    pub(super) fn all_relation_entries(&self) -> Result<Vec<(RelId, RowId, TuplePtr)>> {
        let mut rows = Vec::new();
        for shard in &self.relation_row_dir {
            let shard = shard
                .read()
                .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
            for (rel_id, entries) in shard.iter() {
                rows.extend(entries.iter().map(|(row_id, ptr)| (*rel_id, *row_id, *ptr)));
            }
        }
        Ok(rows)
    }

    pub(crate) fn head(&self, row_id: RowId) -> Result<Option<TuplePtr>> {
        let shard = self.row_dir_shard(row_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        Ok(shard.get(&row_id).copied())
    }

    pub(crate) fn head_for_relation(
        &self,
        rel_id: RelId,
        row_id: RowId,
    ) -> Result<Option<TuplePtr>> {
        let shard = self.relation_row_dir_shard(rel_id);
        let shard = shard
            .read()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        Ok(shard
            .get(&rel_id)
            .and_then(|entries| entries.get(&row_id).copied()))
    }

    pub(crate) fn set_head(&self, row_id: RowId, ptr: TuplePtr) -> Result<()> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        shard.insert(row_id, ptr);
        Ok(())
    }

    pub(crate) fn set_relation_head(
        &self,
        rel_id: RelId,
        row_id: RowId,
        ptr: TuplePtr,
    ) -> Result<()> {
        let shard = self.relation_row_dir_shard(rel_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        shard.entry(rel_id).or_default().insert(row_id, ptr);
        Ok(())
    }

    pub(super) fn remove_head_if(&self, row_id: RowId, expected: TuplePtr) -> Result<bool> {
        let shard = self.row_dir_shard(row_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("row dir shard poisoned"))?;
        if shard.get(&row_id).copied() == Some(expected) {
            shard.remove(&row_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(super) fn remove_relation_head_if(
        &self,
        rel_id: RelId,
        row_id: RowId,
        expected: TuplePtr,
    ) -> Result<bool> {
        let shard = self.relation_row_dir_shard(rel_id);
        let mut shard = shard
            .write()
            .map_err(|_| Error::CorruptPage("relation row dir shard poisoned"))?;
        if let Some(entries) = shard.get_mut(&rel_id)
            && entries.get(&row_id).copied() == Some(expected)
        {
            entries.remove(&row_id);
            if entries.is_empty() {
                shard.remove(&rel_id);
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn lane_for_row(&self, row_id: RowId) -> usize {
        let lane_count = self.row_dir.len().max(1);
        ActiveHeapPlacementPolicy::row_lane(row_id, lane_count)
    }

    fn row_dir_shard(&self, row_id: RowId) -> &RwLock<HashMap<RowId, TuplePtr>> {
        &self.row_dir[self.lane_for_row(row_id)]
    }

    fn relation_row_dir_shard(
        &self,
        rel_id: RelId,
    ) -> &RwLock<HashMap<RelId, HashMap<RowId, TuplePtr>>> {
        let lane =
            ActiveHeapPlacementPolicy::relation_lane(rel_id, self.relation_row_dir.len().max(1));
        &self.relation_row_dir[lane]
    }
}
