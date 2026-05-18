use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwap;
use parking_lot::Mutex;

use super::schema::{SchemaEpoch, SchemaSnapshot};

#[derive(Debug)]
pub struct CatalogManager {
    current: ArcSwap<SchemaSnapshot>,
    version: AtomicU64,
    ddl_lock: Mutex<()>,
}

impl CatalogManager {
    pub fn new(initial: Arc<SchemaSnapshot>) -> Self {
        let version = initial.meta.schema_epoch.0;
        Self {
            current: ArcSwap::from(initial),
            version: AtomicU64::new(version),
            ddl_lock: Mutex::new(()),
        }
    }

    pub fn current(&self) -> Arc<SchemaSnapshot> {
        self.current.load_full()
    }

    pub fn version(&self) -> SchemaEpoch {
        SchemaEpoch(self.version.load(Ordering::Acquire))
    }

    pub fn publish(&self, next: Arc<SchemaSnapshot>) {
        self.version
            .store(next.meta.schema_epoch.0, Ordering::Release);
        self.current.store(next);
    }

    pub fn lock_ddl(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.ddl_lock.lock()
    }
}
