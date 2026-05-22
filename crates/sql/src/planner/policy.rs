#![allow(dead_code)]

use super::{JoinKind, PhysicalKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JoinChoice {
    pub left_rows: f64,
    pub right_rows: f64,
    pub has_indexable_equality: bool,
    pub has_equality: bool,
    pub has_selection: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AggregateChoice {
    pub input_rows: f64,
    pub group_cols: usize,
    pub ordered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SortChoice {
    pub limit: Option<usize>,
}

pub(crate) trait PlannerPolicy {
    fn choose_join_kind(ctx: JoinChoice) -> JoinKind;
    fn aggregate_kind(ctx: AggregateChoice) -> PhysicalKind;
    fn ordering_kind(ctx: SortChoice) -> PhysicalKind;
}

pub(crate) type ActivePlannerPolicy = SqlCurrentPolicy;

pub(crate) struct SqlCurrentPolicy;

impl PlannerPolicy for SqlCurrentPolicy {
    fn choose_join_kind(ctx: JoinChoice) -> JoinKind {
        if ctx.left_rows <= 16.0 || ctx.right_rows <= 16.0 {
            return JoinKind::NestedLoop;
        }
        if !ctx.has_selection {
            return JoinKind::Cross;
        }
        if ctx.has_indexable_equality && (ctx.left_rows <= 256.0 || ctx.right_rows <= 1024.0) {
            return JoinKind::IndexNestedLoop;
        }
        if ctx.has_equality {
            return JoinKind::Hash;
        }
        JoinKind::NestedLoop
    }

    fn aggregate_kind(ctx: AggregateChoice) -> PhysicalKind {
        if ctx.ordered {
            PhysicalKind::StreamingAggregate
        } else {
            PhysicalKind::HashAggregate
        }
    }

    fn ordering_kind(ctx: SortChoice) -> PhysicalKind {
        if ctx
            .limit
            .is_some_and(|limit| limit <= crate::exec::vec::TOPK_LIMIT_THRESHOLD)
        {
            PhysicalKind::TopN
        } else {
            PhysicalKind::Sort
        }
    }
}

pub(crate) struct SqlIndexJoinBiasPolicy;

impl PlannerPolicy for SqlIndexJoinBiasPolicy {
    fn choose_join_kind(ctx: JoinChoice) -> JoinKind {
        if ctx.has_indexable_equality && ctx.right_rows <= 4096.0 {
            return JoinKind::IndexNestedLoop;
        }
        SqlCurrentPolicy::choose_join_kind(ctx)
    }

    fn aggregate_kind(ctx: AggregateChoice) -> PhysicalKind {
        SqlCurrentPolicy::aggregate_kind(ctx)
    }

    fn ordering_kind(ctx: SortChoice) -> PhysicalKind {
        SqlCurrentPolicy::ordering_kind(ctx)
    }
}

pub(crate) struct SqlVectorBatchPolicy;

impl PlannerPolicy for SqlVectorBatchPolicy {
    fn choose_join_kind(ctx: JoinChoice) -> JoinKind {
        SqlCurrentPolicy::choose_join_kind(ctx)
    }

    fn aggregate_kind(ctx: AggregateChoice) -> PhysicalKind {
        SqlCurrentPolicy::aggregate_kind(ctx)
    }

    fn ordering_kind(ctx: SortChoice) -> PhysicalKind {
        if ctx.limit.is_some_and(|limit| limit <= 256) {
            PhysicalKind::TopN
        } else {
            PhysicalKind::Sort
        }
    }
}

pub(crate) struct SqlHashThroughputPolicy;

impl PlannerPolicy for SqlHashThroughputPolicy {
    fn choose_join_kind(ctx: JoinChoice) -> JoinKind {
        if ctx.has_equality && ctx.left_rows.max(ctx.right_rows) >= 512.0 {
            return JoinKind::Hash;
        }
        SqlCurrentPolicy::choose_join_kind(ctx)
    }

    fn aggregate_kind(ctx: AggregateChoice) -> PhysicalKind {
        if ctx.input_rows >= 2048.0 && ctx.group_cols > 0 {
            PhysicalKind::HashAggregate
        } else {
            SqlCurrentPolicy::aggregate_kind(ctx)
        }
    }

    fn ordering_kind(ctx: SortChoice) -> PhysicalKind {
        SqlCurrentPolicy::ordering_kind(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_policy<P: PlannerPolicy>() {
        let join = P::choose_join_kind(JoinChoice {
            left_rows: 4096.0,
            right_rows: 4096.0,
            has_indexable_equality: true,
            has_equality: true,
            has_selection: true,
        });
        assert!(matches!(
            join,
            JoinKind::NestedLoop | JoinKind::IndexNestedLoop | JoinKind::Hash | JoinKind::Cross
        ));

        let aggregate = P::aggregate_kind(AggregateChoice {
            input_rows: 4096.0,
            group_cols: 1,
            ordered: false,
        });
        assert!(matches!(
            aggregate,
            PhysicalKind::HashAggregate | PhysicalKind::StreamingAggregate
        ));

        let ordering = P::ordering_kind(SortChoice { limit: Some(32) });
        assert!(matches!(ordering, PhysicalKind::TopN | PhysicalKind::Sort));
    }

    #[test]
    fn planner_policy_drop_ins_preserve_basic_invariants() {
        audit_policy::<SqlCurrentPolicy>();
        audit_policy::<SqlIndexJoinBiasPolicy>();
        audit_policy::<SqlVectorBatchPolicy>();
        audit_policy::<SqlHashThroughputPolicy>();
    }
}
