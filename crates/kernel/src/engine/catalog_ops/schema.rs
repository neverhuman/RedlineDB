use super::*;

impl Engine {
    pub fn schema_epoch(&self) -> SchemaEpoch {
        self.catalog.version()
    }

    pub fn schema_snapshot(&self) -> Arc<crate::catalog::SchemaSnapshot> {
        self.catalog.current()
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
        let next = apply_alter_table((*self.catalog_snapshot_for_tx(tx)).clone(), spec)?;
        tx.set_pending_schema_snapshot(Arc::new(next));
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
