//! Aggregate-OVER accumulator: SUM / COUNT / AVG / MIN / MAX / TOTAL
//! evaluated over a window-frame slice.

use std::cmp::Ordering;

use crate::value::{SqlValue, compare_values};

#[derive(Clone)]
pub(super) struct Accumulator {
    kind: AccumulatorKind,
    count: i64,
    sum: f64,
    min: Option<SqlValue>,
    max: Option<SqlValue>,
    saw_any: bool,
    is_real: bool,
    int_sum: i64,
    int_sum_overflow: bool,
}

#[derive(Clone, Copy)]
enum AccumulatorKind {
    Count,
    Sum,
    Total,
    Avg,
    Min,
    Max,
    Unknown,
}

impl AccumulatorKind {
    fn from_name(name: &str) -> Self {
        match name {
            "count" => Self::Count,
            "sum" => Self::Sum,
            "total" => Self::Total,
            "avg" => Self::Avg,
            "min" => Self::Min,
            "max" => Self::Max,
            _ => Self::Unknown,
        }
    }
}

impl Accumulator {
    pub(super) fn new(name: &str) -> Self {
        Self {
            kind: AccumulatorKind::from_name(name),
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
            saw_any: false,
            is_real: false,
            int_sum: 0,
            int_sum_overflow: false,
        }
    }

    pub(super) fn push(&mut self, value: SqlValue) {
        match value {
            SqlValue::Null => {}
            ref v => {
                self.saw_any = true;
                self.count += 1;
                self.accumulate_numeric(v);
                self.update_min_max(v);
            }
        }
    }

    fn accumulate_numeric(&mut self, v: &SqlValue) {
        match v {
            SqlValue::Integer(n) => {
                match self.int_sum.checked_add(*n) {
                    Some(s) => self.int_sum = s,
                    None => self.int_sum_overflow = true,
                }
                self.sum += *n as f64;
            }
            SqlValue::Real(n) => {
                self.is_real = true;
                self.sum += *n;
            }
            other => {
                // Best-effort numeric coercion for SUM/AVG.
                // A43: avoid `String::from_utf8_lossy(b)` allocation.
                // Replacement chars don't fit the numeric grammar, so
                // the lossy-parse path returned Err for non-UTF8 blobs
                // and the `if let Ok(n) = parsed` arm below skipped
                // the addition. Borrow on valid UTF-8 via from_utf8;
                // on invalid UTF-8 stay with the same skip semantics
                // by producing a synthetic ParseFloatError (via
                // `"".parse::<f64>()` which is guaranteed-Err and
                // allocation-free). Same shape as A33 / A39 / A41
                // (Blob lossy → from_utf8 short-circuit).
                let parsed = match other {
                    SqlValue::Text(s) => s.parse::<f64>(),
                    SqlValue::Blob(b) => match std::str::from_utf8(b) {
                        Ok(s) => s.parse::<f64>(),
                        Err(_) => "".parse::<f64>(),
                    },
                    _ => Ok(0.0),
                };
                if let Ok(n) = parsed {
                    self.is_real = true;
                    self.sum += n;
                }
            }
        }
    }

    fn update_min_max(&mut self, v: &SqlValue) {
        match &self.min {
            None => self.min = Some(v.clone()),
            Some(cur) if compare_values(v, cur) == Ordering::Less => {
                self.min = Some(v.clone());
            }
            _ => {}
        }
        match &self.max {
            None => self.max = Some(v.clone()),
            Some(cur) if compare_values(v, cur) == Ordering::Greater => {
                self.max = Some(v.clone());
            }
            _ => {}
        }
    }

    pub(super) fn finalize(self) -> SqlValue {
        match self.kind {
            AccumulatorKind::Count => SqlValue::Integer(self.count),
            AccumulatorKind::Sum => {
                if !self.saw_any {
                    return SqlValue::Null;
                }
                if self.is_real || self.int_sum_overflow {
                    SqlValue::Real(self.sum)
                } else {
                    SqlValue::Integer(self.int_sum)
                }
            }
            AccumulatorKind::Total => SqlValue::Real(self.sum),
            AccumulatorKind::Avg => {
                if self.count == 0 {
                    SqlValue::Null
                } else {
                    SqlValue::Real(self.sum / self.count as f64)
                }
            }
            AccumulatorKind::Min => self.min.unwrap_or(SqlValue::Null),
            AccumulatorKind::Max => self.max.unwrap_or(SqlValue::Null),
            AccumulatorKind::Unknown => SqlValue::Null,
        }
    }

    pub(super) fn value(&self) -> SqlValue {
        match self.kind {
            AccumulatorKind::Count => SqlValue::Integer(self.count),
            AccumulatorKind::Sum => {
                if !self.saw_any {
                    return SqlValue::Null;
                }
                if self.is_real || self.int_sum_overflow {
                    SqlValue::Real(self.sum)
                } else {
                    SqlValue::Integer(self.int_sum)
                }
            }
            AccumulatorKind::Total => SqlValue::Real(self.sum),
            AccumulatorKind::Avg => {
                if self.count == 0 {
                    SqlValue::Null
                } else {
                    SqlValue::Real(self.sum / self.count as f64)
                }
            }
            AccumulatorKind::Min => self.min.clone().unwrap_or(SqlValue::Null),
            AccumulatorKind::Max => self.max.clone().unwrap_or(SqlValue::Null),
            AccumulatorKind::Unknown => SqlValue::Null,
        }
    }
}
