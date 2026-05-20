use super::*;

impl Engine {
    pub fn schema_epoch(&self) -> SchemaEpoch {
        self.catalog.version()
    }

    pub fn schema_snapshot(&self) -> Arc<crate::catalog::SchemaSnapshot> {
        self.catalog.current()
    }

    pub fn schema_snapshot_for_tx(&self, tx: &Txn) -> Arc<crate::catalog::SchemaSnapshot> {
        self.catalog_snapshot_for_tx(tx)
    }

    pub fn validate_schema_epoch(&self, epoch: SchemaEpoch) -> Result<()> {
        if self.schema_epoch() == epoch {
            Ok(())
        } else {
            Err(Error::SchemaChanged)
        }
    }

    pub fn sqlite_schema(&self) -> Vec<SqliteSchemaRow> {
        self.catalog.current().sqlite_schema_rows()
    }

    pub fn lookup_table(
        &self,
        tx: &Txn,
        name: crate::catalog::QualifiedName,
    ) -> Result<Arc<crate::catalog::TableDef>> {
        let snapshot = self.catalog_snapshot_for_tx(tx);
        lookup_table(&snapshot, &name)
    }

    pub fn create_table(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateTableSpec,
    ) -> Result<Arc<crate::catalog::TableDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = apply_create_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        let next = Arc::new(next);
        let table = next
            .tables
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created table missing from snapshot"))?;
        tx.set_pending_schema_snapshot(Arc::clone(&next));
        Ok(table)
    }

    pub fn drop_table(&self, tx: &mut Txn, spec: crate::catalog::DropTableSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = apply_drop_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    pub fn alter_table(&self, tx: &mut Txn, spec: crate::catalog::AlterTableSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let current = self.catalog_snapshot_for_tx(tx);
        self.validate_drop_column_storage(&current, &spec)?;
        let next = apply_alter_table((*current).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    fn validate_drop_column_storage(
        &self,
        snapshot: &crate::catalog::SchemaSnapshot,
        spec: &crate::catalog::AlterTableSpec,
    ) -> Result<()> {
        let crate::catalog::AlterTableOperationSpec::DropColumn {
            column_name,
            if_exists: _,
        } = &spec.operation
        else {
            return Ok(());
        };
        let schema_id = crate::catalog::resolve_schema_id(snapshot, Some(&spec.name.schema))?;
        let Some(table) = snapshot.lookup_table(schema_id, spec.name.name.folded()) else {
            return Ok(());
        };
        let Some(drop_ordinal) = table
            .columns
            .iter()
            .position(|column| column.folded.as_ref() == column_name.folded())
        else {
            return Ok(());
        };
        let drops_last_column = drop_ordinal + 1 == table.columns.len();
        if !drops_last_column && !self.relation_entries(table.relation_id)?.is_empty() {
            return Err(Error::UnsupportedDdl(
                "ALTER TABLE DROP COLUMN requires table rewrite when rows exist",
            ));
        }
        Ok(())
    }

    pub fn create_view(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateViewSpec,
    ) -> Result<Arc<crate::catalog::ViewDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next =
            crate::catalog::apply_create_view((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        let next = Arc::new(next);
        let view = next
            .views
            .last()
            .cloned()
            .ok_or(Error::CatalogCorrupt("created view missing from snapshot"))?;
        tx.set_pending_schema_snapshot(Arc::clone(&next));
        Ok(view)
    }

    pub fn drop_view(&self, tx: &mut Txn, spec: crate::catalog::DropViewSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next =
            crate::catalog::apply_drop_view((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }

    pub fn create_trigger(
        &self,
        tx: &mut Txn,
        spec: crate::catalog::CreateTriggerSpec,
    ) -> Result<Arc<crate::catalog::TriggerDef>> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next = crate::catalog::apply_create_trigger(
            (*self.catalog_snapshot_for_tx(tx)).clone(),
            spec,
        )?;
        let next = Arc::new(next);
        let trigger = next.triggers.last().cloned().ok_or(Error::CatalogCorrupt(
            "created trigger missing from snapshot",
        ))?;
        tx.set_pending_schema_snapshot(Arc::clone(&next));
        Ok(trigger)
    }

    pub fn drop_trigger(&self, tx: &mut Txn, spec: crate::catalog::DropTriggerSpec) -> Result<()> {
        tx.ensure_open()?;
        let _ddl = self.catalog.lock_ddl();
        let next =
            crate::catalog::apply_drop_trigger((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
        Ok(())
    }
}
