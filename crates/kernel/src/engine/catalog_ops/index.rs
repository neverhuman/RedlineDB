use super::*;

use crate::catalog::{IndexKeySource, IndexMethod};
use crate::vector::hnsw::{HnswIndex, HnswParams};

impl Engine {
    pub fn create_index(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateIndexSpec,
    ) -> Result<Arc<crate::catalog::IndexDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        // Step 1: build the catalog delta so we know the index_id.
        let method = spec.method;
        let next = apply_create_index((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        let created_index = next
            .indexes
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Phase 2B.1a: dispatch on the kernel index method. B-tree is
        // the historical path; HNSW allocates a graph index page chain
        // and tracks it in the engine's `hnsw_handles` map.
        match method {
            IndexMethod::Btree => self.create_btree_index(tx, &next, &created_index),
            IndexMethod::Hnsw => self.create_hnsw_index(tx, &next, &created_index),
        }
    }

    fn create_btree_index(
        &self,
        tx: &mut Txn,
        next: &crate::catalog::SchemaSnapshot,
        created_index: &Arc<crate::catalog::IndexDef>,
    ) -> Result<Arc<crate::catalog::IndexDef>> {
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
        let btree = BtreeIndex::create_with_wal(
            Arc::clone(&self.buffer),
            descriptor,
            Some(Arc::clone(&self.wal)),
        )?;
        btree.set_phase11_counters(Arc::clone(&self.phase11_counters));
        // Log PageImage records for meta + root so recovery can reconstruct
        // the B-tree even if no checkpoint runs before engine close.
        btree.record_initial_page_images(tx.id())?;
        let meta_page_id = btree.meta_page_id();

        // Step 3: persist meta_page_id back into the snapshot.
        let with_meta =
            apply_set_index_meta_page_id(next.clone(), created_index.index_id, meta_page_id)?;
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

    /// Phase 2B.1a: create an empty HNSW graph index backed by the
    /// shared buffer pool / WAL. The index column must be declared
    /// `VECTOR(N[, f32])`; the dimension is recovered from the
    /// declared-type text. Tuning parameters default to
    /// [`HnswParams::standard`] — the parser does not yet accept
    /// `WITH (m=..., ef_construction=...)` overrides.
    fn create_hnsw_index(
        &self,
        tx: &mut Txn,
        next: &crate::catalog::SchemaSnapshot,
        created_index: &Arc<crate::catalog::IndexDef>,
    ) -> Result<Arc<crate::catalog::IndexDef>> {
        // The HNSW kernel currently indexes a single vector column.
        // Composite keys, expression keys, and DESC sort dirs are
        // meaningless for ANN search and are rejected explicitly so
        // misuse is surfaced at CREATE INDEX time, not at first query.
        if created_index.keys.len() != 1 {
            return Err(Error::UnsupportedDdl(
                "USING hnsw requires exactly one index column",
            ));
        }
        let key = &created_index.keys[0];
        let attnum = match &key.source {
            IndexKeySource::Column { attnum } => *attnum,
            IndexKeySource::Expression { .. } => {
                return Err(Error::UnsupportedDdl(
                    "USING hnsw does not support expression-source index keys",
                ));
            }
        };
        if created_index.unique {
            return Err(Error::UnsupportedDdl(
                "UNIQUE indexes are not supported for USING hnsw",
            ));
        }

        let table = next
            .table_by_id(created_index.table_id)
            .ok_or(Error::ObjectNotFound)?;
        let column = table
            .columns
            .iter()
            .find(|c| c.ordinal == attnum)
            .ok_or(Error::CatalogCorrupt(
                "hnsw index column not found in table",
            ))?;
        let declared = column.declared_type.as_deref().ok_or(Error::UnsupportedDdl(
            "USING hnsw requires the column to be declared VECTOR(N)",
        ))?;
        let dim = parse_vector_declared_dim(declared).ok_or_else(|| {
            Error::Vector(format!(
                "USING hnsw column type '{declared}' is not VECTOR(N) — \
                 declare the column as VECTOR(N) or VECTOR(N, f32)"
            ))
        })?;

        let params = HnswParams::standard(dim as usize);
        // Deterministic seed derived from the index id so reopen of a
        // checkpointed-but-not-replayed index would produce the same
        // level draws. The HNSW kernel persists the seed in its meta
        // page, so this is only the initial value.
        let rng_seed = 0xCAFEF00D_BAD1DEAA ^ created_index.index_id.0;
        let hnsw = HnswIndex::create_with_wal(
            Arc::clone(&self.buffer),
            created_index.relation_id,
            created_index.index_id.0,
            params,
            Some(Arc::clone(&self.wal)),
            rng_seed,
        )?;
        let meta_page_id = hnsw.meta_page_id();

        // Persist meta_page_id back into the snapshot so reopen can
        // locate the HNSW meta page from the catalog (same slot the
        // B-tree uses today).
        let with_meta =
            apply_set_index_meta_page_id(next.clone(), created_index.index_id, meta_page_id)?;
        let with_meta = Arc::new(with_meta);
        let final_index = with_meta
            .index_by_id(created_index.index_id)
            .ok_or(Error::CatalogCorrupt("created index missing from snapshot"))?;

        // Backfill: insert every visible row's vector into the new
        // graph. On an empty table this is a no-op.
        self.backfill_hnsw_index(tx, &hnsw, &table, attnum)?;

        // Install the handle on commit, drop it on rollback. Reusing
        // `PendingIndexHandle::Install` keeps the rollback semantics
        // wired identically to B-tree; HNSW gets its own engine map so
        // a concurrent rollback never sees a half-built graph.
        tx.push_pending_index_handle(PendingIndexHandle::InstallHnsw(
            final_index.index_id,
            Arc::new(hnsw),
        ));
        tx.set_pending_schema_snapshot(Arc::clone(&with_meta));
        Ok(final_index)
    }

    /// Backfill an empty HNSW index by inserting every visible row's
    /// vector. Mirrors `backfill_index` for B-tree but decodes the
    /// blob payload to an `f32` slice before calling
    /// `HnswIndex::insert_tx`.
    fn backfill_hnsw_index(
        &self,
        tx: &mut Txn,
        hnsw: &HnswIndex,
        table: &crate::catalog::TableDef,
        attnum: u16,
    ) -> Result<()> {
        use crate::catalog::{RecordRef, RecordScratch, ValueRef};

        let entries = self.heap.relation_entries(table.relation_id)?;
        if entries.is_empty() {
            return Ok(());
        }
        let mut scratch = RecordScratch::default();
        for (row_id, ptr) in entries {
            let payload = self.heap.get_for_relation(
                &self.txs,
                tx.snapshot(),
                Some(tx.id()),
                table.relation_id,
                row_id,
            )?;
            let Some(payload) = payload else { continue };
            let record = RecordRef::new(&payload)
                .map_err(|_| Error::CorruptPage("hnsw backfill: malformed heap record"))?;
            record
                .decode_into(&mut scratch)
                .map_err(|_| Error::CorruptPage("hnsw backfill: record decode failed"))?;
            let ncols = record
                .column_count()
                .map_err(|_| Error::CorruptPage("hnsw backfill: record decode failed"))?;
            let col_offset = if ncols == table.columns.len() + 1 {
                1
            } else {
                0
            };
            let value = record
                .value_at(&scratch, attnum as usize + col_offset)
                .map_err(|_| Error::CorruptPage("hnsw backfill: column out of range"))?;
            let blob = match value {
                ValueRef::Blob(b) => b,
                ValueRef::Null => continue, // skip nulls; matches SQLite secondary-index semantics
                _ => {
                    return Err(Error::CorruptPage(
                        "hnsw backfill: expected blob in vector column",
                    ));
                }
            };
            let vec = crate::vector::codec::decode_vector(blob)
                .map_err(|_| Error::CorruptPage("hnsw backfill: malformed vector blob"))?;
            let row_ref = crate::vector::hnsw::IndexedRowRef::new(row_id, ptr);
            hnsw.insert_tx(tx.id(), &vec, row_ref)?;
        }
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

    /// Phase 2B.1a: returns the live [`HnswIndex`] handle for the given
    /// catalog `IndexId`, if one has been allocated. Returns `None` for
    /// B-tree indexes — callers should fall back to
    /// [`Engine::index_handle`] when they don't care about the kind.
    pub fn hnsw_index_handle(&self, index_id: CatalogIndexId) -> Option<Arc<HnswIndex>> {
        self.hnsw_handles
            .lock()
            .ok()
            .and_then(|handles| handles.get(&index_id).cloned())
    }

    pub(crate) fn rehydrate_index_handles(self: &Arc<Self>) -> Result<()> {
        let snapshot = self.catalog.current();
        let mut rebuilt = Vec::new();
        let mut opened = Vec::new();
        let mut hnsw_opened: Vec<(CatalogIndexId, Arc<HnswIndex>)> = Vec::new();
        for index in &snapshot.indexes {
            let Some(meta_page_id) = index.meta_page_id else {
                // Pre-Lane-A index without physical pages; nothing to reopen.
                continue;
            };
            // Phase 2B.1a: dispatch on the persisted method tag rather
            // than inferring kind from page magic — the catalog is the
            // source of truth, and per-index dispatch keeps the B-tree
            // and HNSW reopen paths cleanly separated.
            match index.method {
                IndexMethod::Hnsw => {
                    let hnsw = HnswIndex::open_with_wal(
                        Arc::clone(&self.buffer),
                        meta_page_id,
                        Some(Arc::clone(&self.wal)),
                    )?;
                    hnsw_opened.push((index.index_id, Arc::new(hnsw)));
                    continue;
                }
                IndexMethod::Btree => {}
            }
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
                    Some(Arc::clone(&self.wal)),
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
            let btree = BtreeIndex::create_with_wal(
                Arc::clone(&self.buffer),
                descriptor,
                Some(Arc::clone(&self.wal)),
            )?;
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
        drop(handles);
        if !hnsw_opened.is_empty() {
            let mut hnsw_handles = self
                .hnsw_handles
                .lock()
                .map_err(|_| Error::CorruptPage("engine hnsw handles mutex poisoned"))?;
            for (index_id, hnsw) in hnsw_opened {
                hnsw_handles.insert(index_id, hnsw);
            }
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

/// Phase 2B.1a: extract the `N` dimension from a stored declared-type
/// string of the form `VECTOR(N)` or `VECTOR(N, f32)`. Mirrors the SQL
/// parser's `parse_vector_declared_type` helper, but operates on the
/// already-stored string so the kernel can resolve a vector dim
/// without depending on sqlparser. Whitespace and case-insensitive on
/// the leading `VECTOR` token. Returns `None` for any other shape.
fn parse_vector_declared_dim(declared: &str) -> Option<u32> {
    let trimmed = declared.trim();
    let (head, rest) = trimmed.split_once('(')?;
    if !head.trim().eq_ignore_ascii_case("VECTOR") {
        return None;
    }
    let body = rest.strip_suffix(')')?;
    let mut parts = body.split(',').map(|s| s.trim());
    let dim_str = parts.next()?;
    let dim: u32 = dim_str.parse().ok()?;
    if dim == 0 {
        return None;
    }
    // Optional element-kind argument; only `f32` is supported today,
    // which matches the SQL parser's acceptance gate.
    if let Some(kind) = parts.next() {
        if !kind.eq_ignore_ascii_case("f32") {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    Some(dim)
}

#[cfg(test)]
mod tests {
    use super::parse_vector_declared_dim;

    #[test]
    fn extracts_dim_from_canonical_vector_type() {
        assert_eq!(parse_vector_declared_dim("VECTOR(384)"), Some(384));
        assert_eq!(parse_vector_declared_dim("VECTOR(8, f32)"), Some(8));
        assert_eq!(parse_vector_declared_dim("vector(16,F32)"), Some(16));
        assert_eq!(parse_vector_declared_dim("  VECTOR(4)  "), Some(4));
    }

    #[test]
    fn rejects_non_vector_types_and_bad_dims() {
        assert_eq!(parse_vector_declared_dim("INTEGER"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR(0)"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR(4, i8)"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR(4, f32, x)"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR()"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR(abc)"), None);
        assert_eq!(parse_vector_declared_dim("VECTOR("), None);
        assert_eq!(parse_vector_declared_dim("BLOB(4)"), None);
    }
}
