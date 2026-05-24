use super::*;

impl Engine {
    pub fn create_index(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateIndexSpec,
    ) -> Result<Arc<crate::catalog::IndexDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let snapshot = self.catalog_snapshot_for_tx(tx);
        if spec.if_not_exists {
            let schema_id = crate::catalog::resolve_schema_id(&snapshot, spec.schema.as_ref())?;
            if let Some(existing) = snapshot.lookup_index(schema_id, spec.name.folded()) {
                return Ok(existing);
            }
        }
        // Step 1: build the catalog delta so we know the index_id.
        let next = apply_create_index((*snapshot).clone(), spec)?;
        let created_index = next
            .indexes
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Step 2: allocate physical B-tree pages with the WAL coordinator.
        let descriptor = IndexDescriptor::new(
            PhysicalIndexId(created_index.index_id.0),
            created_index.relation_id,
            if created_index.unique {
                IndexUniqueness::Unique
            } else {
                IndexUniqueness::NonUnique
            },
        );
        let btree =
            BtreeIndex::create_with_wal(Arc::clone(&self.buffer), descriptor, self.page_wal())?;
        btree.set_phase11_counters(Arc::clone(&self.phase11_counters));
        // Log PageImage records for meta + root so recovery can reconstruct
        // the B-tree even if no checkpoint runs before engine close.
        btree.record_initial_page_images(tx.id())?;
        let meta_page_id = btree.meta_page_id();

        // Step 3: persist meta_page_id back into the snapshot.
        let with_meta = apply_set_index_meta_page_id(next, created_index.index_id, meta_page_id)?;
        let with_meta = Arc::new(with_meta);
        let final_index = with_meta
            .index_by_id(created_index.index_id)
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Step 4: DDL backfill — index every visible row of the underlying
        // table at the time of CREATE INDEX. The backfill uses the in-memory
        // snapshot/tx_status; if the table is empty this is a no-op.
        let table = with_meta
            .table_by_id(final_index.table_id)
            .ok_or(Error::ObjectNotFound)?;
        self.backfill_index(tx, &btree, &table, &final_index)?;

        // Step 5: install the handle only if the surrounding DDL transaction
        // commits. Rollback must not expose a handle for a catalog entry that
        // never became visible.
        tx.push_pending_index_handle(PendingIndexHandle::Install(
            final_index.index_id,
            Arc::new(btree),
        ));
        tx.set_pending_schema_snapshot(Arc::clone(&with_meta));
        Ok(final_index)
    }

    /// Track J — rename an index in place. The underlying B-tree is keyed by
    /// `index_id`, so no physical work is required; only the catalog `name`
    /// / `folded` fields update. Returns an error if `old_folded` does not
    /// resolve to an index or `new_name` already exists.
    pub fn rename_index(&self, tx: &mut Txn, old_folded: &str, new_name: &str) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let snapshot = self.catalog_snapshot_for_tx(tx);
        let next = crate::catalog::apply_rename_index((*snapshot).clone(), old_folded, new_name)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    pub fn drop_index(&self, tx: &mut Txn, spec: crate::catalog::DropIndexSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        // Find the index id BEFORE applying the drop (the snapshot mutates).
        let snapshot = self.catalog_snapshot_for_tx(tx);
        let removed_id = crate::catalog::lookup_index(&snapshot, &spec.name)
            .ok()
            .map(|idx| idx.index_id);

        let next = apply_drop_index((*snapshot).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));

        // Page reuse: PageBackedHeap currently does not support marking
        // arbitrary index meta/root pages as reusable (it tracks Heap/Undo
        // kinds only). The pages remain allocated until vacuum/checkpoint
        // reclaims them via the dedicated btree-reclamation work item:
        // wire btree page reclamation through PageBackedHeap once it
        // supports BtreeMeta and BtreeLeaf reusability.
        if let Some(index_id) = removed_id {
            tx.push_pending_index_handle(PendingIndexHandle::Remove(index_id));
        }
        Ok(())
    }

    /// Returns the live `BtreeIndex` handle for the given catalog `IndexId`,
    /// if one has been allocated. SQL exec lanes (B/C) use this to issue
    /// physical lookups and maintenance operations against the index.
    pub fn index_handle(&self, index_id: CatalogIndexId) -> Option<Arc<BtreeIndex>> {
        self.index_handles
            .lock()
            .ok()
            .and_then(|handles| handles.get(&index_id).cloned())
    }

    pub(crate) fn rehydrate_index_handles(self: &Arc<Self>) -> Result<()> {
        let snapshot = self.catalog.current();
        let mut rebuilt = Vec::new();
        let mut opened = Vec::new();
        for index in &snapshot.indexes {
            let Some(meta_page_id) = index.meta_page_id else {
                // Pre-Lane-A index without physical pages; nothing to reopen.
                continue;
            };
            let descriptor = IndexDescriptor::new(
                PhysicalIndexId(index.index_id.0),
                index.relation_id,
                if index.unique {
                    IndexUniqueness::Unique
                } else {
                    IndexUniqueness::NonUnique
                },
            );
            let version = BtreeIndex::format_version(&self.buffer, meta_page_id)?;
            if version == INDEX_VERSION {
                let btree = BtreeIndex::open_with_wal(
                    Arc::clone(&self.buffer),
                    meta_page_id,
                    descriptor,
                    self.page_wal(),
                )?;
                btree.set_phase11_counters(Arc::clone(&self.phase11_counters));
                opened.push((index.index_id, Arc::new(btree)));
            } else if version == 1 {
                let table = snapshot
                    .table_by_id(index.table_id)
                    .ok_or(Error::CatalogCorrupt("index table missing during rebuild"))?;
                rebuilt.push((index.as_ref().clone(), table));
            } else {
                return Err(Error::UnsupportedVersion(version));
            }
        }
        let mut next_snapshot = (*snapshot).clone();
        let mut rebuild_tx = if rebuilt.is_empty() {
            None
        } else {
            Some(self.begin(Isolation::Snapshot)?)
        };
        for (index, table) in rebuilt {
            let descriptor = IndexDescriptor::new(
                PhysicalIndexId(index.index_id.0),
                index.relation_id,
                if index.unique {
                    IndexUniqueness::Unique
                } else {
                    IndexUniqueness::NonUnique
                },
            );
            let btree =
                BtreeIndex::create_with_wal(Arc::clone(&self.buffer), descriptor, self.page_wal())?;
            btree.set_phase11_counters(Arc::clone(&self.phase11_counters));
            let tx = rebuild_tx
                .as_mut()
                .ok_or(Error::CorruptPage("missing index rebuild transaction"))?;
            btree.record_initial_page_images(tx.id())?;
            self.backfill_index(tx, &btree, &table, &index)?;
            next_snapshot =
                apply_set_index_meta_page_id(next_snapshot, index.index_id, btree.meta_page_id())?;
            opened.push((index.index_id, Arc::new(btree)));
        }
        if let Some(mut tx) = rebuild_tx {
            let next_snapshot = Arc::new(next_snapshot);
            tx.set_pending_schema_snapshot(next_snapshot);
            match self.commit(tx)? {
                CommitOutcome::Committed(_) => {}
                CommitOutcome::MaybeCommitted => {
                    return Err(Error::CorruptWal("index rebuild maybe committed"));
                }
                CommitOutcome::RolledBack => {
                    return Err(Error::CorruptWal("index rebuild rolled back"));
                }
            }
        }
        let mut handles = self
            .index_handles
            .lock()
            .map_err(|_| Error::CorruptPage("engine index handles mutex poisoned"))?;
        for (index_id, btree) in opened {
            handles.insert(index_id, btree);
        }
        Ok(())
    }

    /// Walks the heap relation backing this index's table and inserts every
    /// visible row into the freshly-built B-tree. Called from `create_index`
    /// to make the index immediately usable for the rest of the transaction.
    /// On a non-empty table this performs the SQLite-style synchronous
    /// CREATE INDEX backfill. On an empty table it is a no-op.
    fn backfill_index(
        &self,
        tx: &mut Txn,
        btree: &BtreeIndex,
        table: &crate::catalog::TableDef,
        index: &crate::catalog::IndexDef,
    ) -> Result<()> {
        use crate::catalog::{
            EncodedIndexKey, IndexKeySource, RecordRef, RecordScratch, ValueRef, encode_index_key,
        };

        // Snapshot the row directory for this relation BEFORE we begin so the
        // backfill does not race with concurrent inserts in the same tx.
        let entries = self.heap.relation_entries(table.relation_id)?;
        if entries.is_empty() {
            return Ok(());
        }
        let mut scratch = RecordScratch::default();
        let mut key_buf = Vec::new();
        let dirs: Vec<crate::catalog::SortDir> =
            index.keys.iter().map(|key| key.sort_dir).collect();
        for (row_id, _ptr) in entries {
            let payload = self.heap.get_for_relation(
                &self.txs,
                tx.snapshot(),
                Some(tx.id()),
                table.relation_id,
                row_id,
            )?;
            let Some(payload) = payload else {
                continue;
            };
            let record = RecordRef::new(&payload)
                .map_err(|_| Error::CorruptPage("index backfill: malformed heap record"))?;
            record
                .decode_into(&mut scratch)
                .map_err(|_| Error::CorruptPage("index backfill: record decode failed"))?;
            // SQL-encoded rows (encode_sql_row) prepend table_id at col 0;
            // kernel-direct rows (encode_record) do not. Detect by comparing
            // the record column count against the table's user column count.
            let ncols = record
                .column_count()
                .map_err(|_| Error::CorruptPage("index backfill: record decode failed"))?;
            let col_offset = if ncols == table.columns.len() + 1 {
                1
            } else {
                0
            };
            let mut parts: Vec<ValueRef<'_>> = Vec::with_capacity(index.keys.len());
            let mut has_expression_key = false;
            for key in &index.keys {
                let attnum = match &key.source {
                    IndexKeySource::Column { attnum } => *attnum,
                    IndexKeySource::Expression { .. } => {
                        // Kernel cannot evaluate SQL expressions; the SQL
                        // layer is the source of truth for expression
                        // index maintenance.
                        has_expression_key = true;
                        break;
                    }
                };
                let value = record
                    .value_at(&scratch, attnum as usize + col_offset)
                    .map_err(|_| Error::CorruptPage("index backfill: column out of range"))?;
                parts.push(value);
            }
            if has_expression_key {
                continue;
            }
            let EncodedIndexKey {
                bytes,
                contains_null,
            } = encode_index_key(&parts, &dirs, &mut key_buf);
            // SQLite NULL-uniqueness rule: skip the unique conflict check
            // when any leading key component is NULL — duplicates of NULL
            // are allowed in unique indexes.
            if index.unique && !contains_null {
                let owner = tx.id().0;
                let _guard = btree.lock_unique_key(owner, &bytes)?;
                if !btree
                    .point_lookup_visible(&self.txs, tx.snapshot(), Some(tx.id()), &bytes)?
                    .is_empty()
                {
                    return Err(Error::WriteConflict);
                }
            }
            let row_ref = IndexRowRef::with_row_id(
                row_id,
                TuplePtr::new_with_generation(PageId(0), 0, PageGeneration::ONE),
            );
            btree.insert_tx(tx.id(), &bytes, row_ref)?;
        }
        Ok(())
    }
}
