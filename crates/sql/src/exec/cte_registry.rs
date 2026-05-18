//! Thread-local row store backing synthetic CTE and view table defs.
//!
//! Statements always bind and execute on the same thread, so a per-thread
//! store avoids lock contention and keeps concurrent tests from trampling
//! each other's id space.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::value::SqlValue;
use redlinedb_kernel::format::RelId;

thread_local! {
    /// Per-thread registry of CTE and view row stores keyed by synthesized
    /// relation_id. Statements always bind and execute on the same thread.
    pub(super) static CTE_ROWS_TL: std::cell::RefCell<HashMap<u64, Arc<Vec<Vec<SqlValue>>>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Reserved global slot — CTE row storage itself lives in `CTE_ROWS_TL`.
#[allow(dead_code)]
static _CTE_ROWS_GLOBAL_RESERVED: Mutex<()> = Mutex::new(());

/// Look up the rows backing a synthetic CTE or view TableDef.
pub(crate) fn rows_for_relation(rel: RelId) -> Option<Arc<Vec<Vec<SqlValue>>>> {
    CTE_ROWS_TL.with(|cell| cell.borrow().get(&rel.0).cloned())
}

pub(crate) fn register_cte_rows(rel: RelId, rows: Arc<Vec<Vec<SqlValue>>>) {
    CTE_ROWS_TL.with(|cell| {
        cell.borrow_mut().insert(rel.0, rows);
    });
}

/// Allow non-CTE callers (e.g. the view module) to publish synthetic
/// row sets into the same per-thread registry.
pub(crate) fn register_external_rows(rel: RelId, rows: Arc<Vec<Vec<SqlValue>>>) {
    register_cte_rows(rel, rows);
}

/// Remove a synthetic row set from the registry by relation id.
pub(crate) fn deregister_rows(rel: RelId) {
    CTE_ROWS_TL.with(|cell| {
        cell.borrow_mut().remove(&rel.0);
    });
}

#[allow(dead_code)]
pub(super) fn release_scope_rows(scope: &HashMap<String, super::CteDef>) {
    CTE_ROWS_TL.with(|cell| {
        let mut map = cell.borrow_mut();
        for def in scope.values() {
            if let Some(tdef) = def.table_def.as_ref() {
                map.remove(&tdef.relation_id.0);
            }
        }
    });
}
