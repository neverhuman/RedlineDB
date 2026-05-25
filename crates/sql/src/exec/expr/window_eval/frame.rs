//! Window-frame resolution and bound computation.

use sqlparser::ast::{Expr, WindowFrameBound, WindowFrameUnits, WindowSpec};

#[derive(Clone, Debug)]
pub(super) struct ResolvedFrame {
    pub(super) units: WindowFrameUnits,
    pub(super) start: ResolvedBound,
    pub(super) end: ResolvedBound,
    pub(super) exclude: ExcludeMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ExcludeMode {
    /// EXCLUDE NO OTHERS (the default).
    #[default]
    NoOthers,
    /// EXCLUDE CURRENT ROW.
    CurrentRow,
    /// EXCLUDE GROUP.
    Group,
    /// EXCLUDE TIES.
    Ties,
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
    let exclude = exclude_from_partition(&window.partition_by);
    match &window.window_frame {
        Some(frame) => ResolvedFrame {
            units: frame.units,
            start: resolve_bound(&frame.start_bound),
            end: match &frame.end_bound {
                Some(end) => resolve_bound(end),
                None => ResolvedBound::CurrentRow,
            },
            exclude,
        },
        None => {
            if window.order_by.is_empty() {
                // No ORDER BY: entire partition.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::UnboundedFollowing,
                    exclude,
                }
            } else {
                // ORDER BY present: RANGE UNBOUNDED PRECEDING -> CURRENT ROW.
                ResolvedFrame {
                    units: WindowFrameUnits::Range,
                    start: ResolvedBound::UnboundedPreceding,
                    end: ResolvedBound::CurrentRow,
                    exclude,
                }
            }
        }
    }
}

/// Inspects PARTITION BY exprs for the magic EXCLUDE-mode marker
/// injected by `parser::rewrite_window_exclude`. Returns the implied
/// `ExcludeMode` (or `NoOthers` if no marker is present).
pub(super) fn exclude_from_partition(partition_by: &[Expr]) -> ExcludeMode {
    for expr in partition_by {
        if let Some(mode) = marker_to_mode(expr) {
            return mode;
        }
    }
    ExcludeMode::NoOthers
}

fn marker_to_mode(expr: &Expr) -> Option<ExcludeMode> {
    let lit = match expr {
        Expr::Value(v) => match &v.value {
            sqlparser::ast::Value::SingleQuotedString(s)
            | sqlparser::ast::Value::DoubleQuotedString(s) => s.as_str(),
            _ => return None,
        },
        _ => return None,
    };
    match lit {
        "__redline_exc_current_row__" => Some(ExcludeMode::CurrentRow),
        "__redline_exc_group__" => Some(ExcludeMode::Group),
        "__redline_exc_ties__" => Some(ExcludeMode::Ties),
        "__redline_exc_no_others__" => Some(ExcludeMode::NoOthers),
        _ => None,
    }
}

/// True if this expression is a EXCLUDE-mode marker literal injected by
/// the parser preprocessor.
pub(super) fn is_exclude_marker(expr: &Expr) -> bool {
    marker_to_mode(expr).is_some()
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
    peer_ranges: &[(usize, usize)],
    total: usize,
) -> (usize, usize) {
    let s = match &frame.start {
        ResolvedBound::UnboundedPreceding => 0i64,
        ResolvedBound::Preceding(n) => sorted_pos as i64 - *n,
        ResolvedBound::CurrentRow => match frame.units {
            WindowFrameUnits::Range | WindowFrameUnits::Groups => {
                let target = peer_ids[sorted_pos];
                peer_ranges
                    .get(target)
                    .map(|range| range.0)
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
                let target = peer_ids[sorted_pos];
                peer_ranges
                    .get(target)
                    .map(|range| range.1 as i64)
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
