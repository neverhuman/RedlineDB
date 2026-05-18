//! Window-frame resolution and bound computation.

use sqlparser::ast::{Expr, WindowFrameBound, WindowFrameUnits, WindowSpec};

#[derive(Clone, Debug)]
pub(super) struct ResolvedFrame {
    pub(super) units: WindowFrameUnits,
    pub(super) start: ResolvedBound,
    pub(super) end: ResolvedBound,
}

#[derive(Clone, Debug)]
pub(super) enum ResolvedBound {
    UnboundedPreceding,
    Preceding(i64),
    CurrentRow,
    Following(i64),
    UnboundedFollowing,
}

pub(super) fn resolve_frame(window: &WindowSpec) -> ResolvedFrame {
    match &window.window_frame {
        Some(frame) => ResolvedFrame {
            units: frame.units,
            start: resolve_bound(&frame.start_bound),
            end: match &frame.end_bound {
                Some(end) => resolve_bound(end),
                None => ResolvedBound::CurrentRow,
            },
        },
        None => {
            if window.order_by.is_empty() {
                // No ORDER BY: entire partition.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::UnboundedFollowing,
                }
            } else {
                // ORDER BY present: RANGE UNBOUNDED PRECEDING -> CURRENT ROW.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::CurrentRow,
                }
            }
        }
    }
}

fn resolve_bound(bound: &WindowFrameBound) -> ResolvedBound {
    match bound {
        WindowFrameBound::CurrentRow => ResolvedBound::CurrentRow,
        WindowFrameBound::Preceding(None) => ResolvedBound::UnboundedPreceding,
        WindowFrameBound::Following(None) => ResolvedBound::UnboundedFollowing,
        WindowFrameBound::Preceding(Some(expr)) => match literal_i64(expr) {
            Some(n) => ResolvedBound::Preceding(n),
            None => ResolvedBound::Preceding(0),
        },
        WindowFrameBound::Following(Some(expr)) => match literal_i64(expr) {
            Some(n) => ResolvedBound::Following(n),
            None => ResolvedBound::Following(0),
        },
    }
}

pub(super) fn literal_i64(expr: &Expr) -> Option<i64> {
    if let Expr::Value(v) = expr
        && let sqlparser::ast::Value::Number(s, _) = &v.value
    {
        return s.parse::<i64>().ok();
    }
    None
}

/// Compute (start, end) sorted-position bounds for the row at
/// `sorted_pos` under `frame`. End is inclusive. Returns positions
/// clamped into `[0, total-1]`. May return start > end (empty frame).
pub(super) fn frame_bounds(
    frame: &ResolvedFrame,
    sorted_pos: usize,
    peer_ids: &[usize],
    total: usize,
) -> (usize, usize) {
    let s = match &frame.start {
        ResolvedBound::UnboundedPreceding => 0i64,
        ResolvedBound::Preceding(n) => sorted_pos as i64 - *n,
        ResolvedBound::CurrentRow => match frame.units {
            WindowFrameUnits::Range | WindowFrameUnits::Groups => {
                // First row of the current peer group.
                let target = peer_ids[sorted_pos];
                peer_ids
                    .iter()
                    .position(|&id| id == target)
                    .unwrap_or(sorted_pos) as i64
            }
            WindowFrameUnits::Rows => sorted_pos as i64,
        },
        ResolvedBound::Following(n) => sorted_pos as i64 + *n,
        ResolvedBound::UnboundedFollowing => total as i64,
    };
    let e = match &frame.end {
        ResolvedBound::UnboundedPreceding => -1i64,
        ResolvedBound::Preceding(n) => sorted_pos as i64 - *n,
        ResolvedBound::CurrentRow => match frame.units {
            WindowFrameUnits::Range | WindowFrameUnits::Groups => {
                // Last row of the current peer group.
                let target = peer_ids[sorted_pos];
                peer_ids
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|&(_, &id)| id == target)
                    .map(|(i, _)| i as i64)
                    .unwrap_or(sorted_pos as i64)
            }
            WindowFrameUnits::Rows => sorted_pos as i64,
        },
        ResolvedBound::Following(n) => sorted_pos as i64 + *n,
        ResolvedBound::UnboundedFollowing => total as i64 - 1,
    };
    let s = s.max(0) as usize;
    let e = if e < 0 { 0 } else { e as usize };
    let e = e.min(total.saturating_sub(1));
    (s, e)
}
