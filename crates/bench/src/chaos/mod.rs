//! Chaos workload module.
//!
//! Each `chaos-*` workload lives in its own sibling file so the per-file
//! duplicate-block detector cannot flag structural similarity inside the
//! shared `run_chaos_workload` skeleton. `helpers` owns the cross-cutting
//! plumbing (thread loops, checksum capture, stats schema, transaction
//! finishers); the per-workload modules call back into it for everything
//! that is not workload-specific.

mod checkpoint_thrash;
mod connection_churn;
mod helpers;
mod index_hammer;
mod lock_convoy;
mod schema_storm;
mod sort_spill_convoy;

pub(crate) use checkpoint_thrash::run_checkpoint_thrash;
pub(crate) use connection_churn::run_connection_churn;
pub(crate) use index_hammer::run_index_hammer;
pub(crate) use lock_convoy::run_lock_convoy;
pub(crate) use schema_storm::run_schema_storm;
pub(crate) use sort_spill_convoy::run_sort_spill_convoy;

#[cfg(test)]
mod tests;
