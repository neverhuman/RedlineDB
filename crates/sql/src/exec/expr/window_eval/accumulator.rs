//! Aggregate-OVER accumulator: SUM / COUNT / AVG / MIN / MAX / TOTAL
//! evaluated over a window-frame slice.

use std::cmp::Ordering;

use crate::value::{SqlValue, compare_values};

pub(super) struct Accumulator {
    kind: String,
    count: i64,
    sum: f64,
    min: Option<SqlValue>,
    max: Option<SqlValue>,
    saw_any: bool,
    is_real: bool,
    int_sum: i64,
    int_sum_overflow: bool,
}

impl Accumulator {
    pub(super) fn new(name: &str) -> Self {
        Self {
            kind: name.to_owned(),
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
                let parsed = match other {
                    SqlValue::Text(s) => s.parse::<f64>(),
                    SqlValue::Blob(b) => String::from_utf8_lossy(b).parse::<f64>(),
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
        match self.kind.as_str() {
            "count" => SqlValue::Integer(self.count),
            "sum" => {
                if !self.saw_any {
                    return SqlValue::Null;
                }
                if self.is_real || self.int_sum_overflow {
                    SqlValue::Real(self.sum)
                } else {
                    SqlValue::Integer(self.int_sum)
                }
            }
            "total" => SqlValue::Real(self.sum),
            "avg" => {
                if self.count == 0 {
                    SqlValue::Null
                } else {
                    SqlValue::Real(self.sum / self.count as f64)
                }
            }
            "min" => self.min.unwrap_or(SqlValue::Null),
            "max" => self.max.unwrap_or(SqlValue::Null),
            _ => SqlValue::Null,
        }
    }
}
