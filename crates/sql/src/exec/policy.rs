#![allow(dead_code)]

pub(crate) trait ExecBatchPolicy {
    const ROW_BATCH_ROWS: usize;
    const INDEX_ROWID_BATCH: usize;

    fn materialize_rows(row_width: usize, memory_budget: usize) -> usize;
}

pub(crate) type ActiveExecBatchPolicy = SqlCurrentExecPolicy;

pub(crate) struct SqlCurrentExecPolicy;

impl ExecBatchPolicy for SqlCurrentExecPolicy {
    const ROW_BATCH_ROWS: usize = 1024;
    const INDEX_ROWID_BATCH: usize = 256;

    fn materialize_rows(row_width: usize, memory_budget: usize) -> usize {
        if row_width == 0 {
            return Self::ROW_BATCH_ROWS;
        }
        (memory_budget / row_width).clamp(1, Self::ROW_BATCH_ROWS)
    }
}

pub(crate) struct SqlVectorBatchExecPolicy;

impl ExecBatchPolicy for SqlVectorBatchExecPolicy {
    const ROW_BATCH_ROWS: usize = 4096;
    const INDEX_ROWID_BATCH: usize = 1024;

    fn materialize_rows(row_width: usize, memory_budget: usize) -> usize {
        if row_width == 0 {
            return Self::ROW_BATCH_ROWS;
        }
        (memory_budget / row_width).clamp(1, Self::ROW_BATCH_ROWS)
    }
}

pub(crate) struct SqlIndexJoinBiasExecPolicy;

impl ExecBatchPolicy for SqlIndexJoinBiasExecPolicy {
    const ROW_BATCH_ROWS: usize = 1024;
    const INDEX_ROWID_BATCH: usize = 512;

    fn materialize_rows(row_width: usize, memory_budget: usize) -> usize {
        SqlCurrentExecPolicy::materialize_rows(row_width, memory_budget)
    }
}

pub(crate) struct SqlMemoryBoundExecPolicy;

impl ExecBatchPolicy for SqlMemoryBoundExecPolicy {
    const ROW_BATCH_ROWS: usize = 512;
    const INDEX_ROWID_BATCH: usize = 128;

    fn materialize_rows(row_width: usize, memory_budget: usize) -> usize {
        if row_width == 0 {
            return Self::ROW_BATCH_ROWS;
        }
        (memory_budget / row_width).clamp(1, Self::ROW_BATCH_ROWS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_policy<P: ExecBatchPolicy>() {
        assert!(P::ROW_BATCH_ROWS > 0);
        assert!(P::INDEX_ROWID_BATCH > 0);
        assert!(P::materialize_rows(8, 64) >= 1);
        assert!(P::materialize_rows(8, usize::MAX) <= P::ROW_BATCH_ROWS);
    }

    #[test]
    fn exec_batch_drop_ins_preserve_basic_invariants() {
        audit_policy::<SqlCurrentExecPolicy>();
        audit_policy::<SqlVectorBatchExecPolicy>();
        audit_policy::<SqlIndexJoinBiasExecPolicy>();
        audit_policy::<SqlMemoryBoundExecPolicy>();
    }
}
