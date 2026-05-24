use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::super::*;
use super::order::eval_group_key;
use super::select::expr_contains_aggregate;

pub(super) fn try_execute_simple_grouped_aggregate(
    plan: &crate::statement::SelectPlan,
    rows: &[SqlRow],
    bindings: &[Option<SqlValue>],
    limit: usize,
    offset: usize,
    memory: &mut QueryMemoryBroker,
) -> Result<Option<Vec<Vec<SqlValue>>>> {
    if plan.distinct || projection_has_wildcard(&plan.projection) {
        return Ok(None);
    }
    if plan.group_by.is_empty() && !projection_is_aggregate_only(&plan.projection) {
        return Ok(None);
    }

    let mut collector = AggCollector::default();
    for item in &plan.projection {
        if !collector.collect_select_item(item) {
            return Ok(None);
        }
    }
    if let Some(having) = &plan.having
        && !collector.collect_expr(having)
    {
        return Ok(None);
    }
    for order in &plan.order_by {
        if !collector.collect_expr(&order.expr) {
            return Ok(None);
        }
    }
    let specs = collector.specs;

    let mut groups: Vec<SimpleGroup> = Vec::new();
    let mut index_by_key: HashMap<Vec<u8>, usize> = HashMap::with_capacity(rows.len());
    for row in rows {
        if !selection_passes(&plan.selection, row, bindings)? {
            continue;
        }
        let key = eval_group_key(&plan.group_by, row, bindings)?;
        let key_bytes = vec::hash_agg::encode_group_key_bytes(&key)?;
        let group_idx = match index_by_key.get(&key_bytes) {
            Some(&idx) => idx,
            None => {
                let idx = groups.len();
                index_by_key.insert(key_bytes, idx);
                groups.push(SimpleGroup::new(row.clone(), &specs));
                idx
            }
        };
        groups[group_idx].observe(row, &specs, bindings)?;
    }
    if groups.is_empty() && plan.group_by.is_empty() {
        groups.push(SimpleGroup::new(SqlRow::Empty, &specs));
    }

    let mut projected = Vec::with_capacity(groups.len());
    let mut order_keys = Vec::with_capacity(groups.len());
    for group in &groups {
        let values = group.finalize();
        if let Some(having) = &plan.having
            && !is_truthy(&eval_simple_expr(having, group, &values, &specs, bindings)?)
        {
            continue;
        }
        let row = project_simple_group(&plan.projection, group, &values, &specs, bindings)?;
        if !plan.order_by.is_empty() {
            order_keys.push(eval_order_keys(
                &plan.projection,
                &plan.order_by,
                &row,
                group,
                &values,
                &specs,
                bindings,
            )?);
        }
        projected.push(row);
    }

    if !plan.order_by.is_empty() {
        sort_projected_with_keys(&mut projected, &mut order_keys, &plan.order_by);
    }

    let memory_bytes = projected
        .iter()
        .try_fold(0usize, |acc, row| Ok::<usize, Error>(acc + row_width(row)))?;
    memory.request(memory_bytes)?;
    Ok(Some(
        projected.into_iter().skip(offset).take(limit).collect(),
    ))
}

#[derive(Default)]
struct AggCollector {
    specs: Vec<SimpleAggSpec>,
    index_by_signature: HashMap<String, usize>,
}

impl AggCollector {
    fn collect_select_item(&mut self, item: &SelectItem) -> bool {
        match item {
            SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                self.collect_expr(expr)
            }
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
        }
    }

    fn collect_expr(&mut self, expr: &Expr) -> bool {
        if !expr_contains_aggregate(expr) {
            return true;
        }
        match expr {
            Expr::Function(func) => match simple_agg_spec(func) {
                AggMatch::Aggregate(spec) => {
                    self.add_spec(spec);
                    true
                }
                AggMatch::Scalar => function_exprs(func)
                    .is_some_and(|exprs| exprs.into_iter().all(|expr| self.collect_expr(expr))),
                AggMatch::Unsupported => false,
            },
            Expr::BinaryOp { left, right, .. } => {
                self.collect_expr(left) && self.collect_expr(right)
            }
            Expr::UnaryOp { expr, .. }
            | Expr::Nested(expr)
            | Expr::IsNull(expr)
            | Expr::IsNotNull(expr)
            | Expr::IsTrue(expr)
            | Expr::IsNotTrue(expr)
            | Expr::IsFalse(expr)
            | Expr::IsNotFalse(expr)
            | Expr::IsUnknown(expr)
            | Expr::IsNotUnknown(expr) => self.collect_expr(expr),
            Expr::Cast { expr, .. } => self.collect_expr(expr),
            Expr::Between {
                expr, low, high, ..
            } => self.collect_expr(expr) && self.collect_expr(low) && self.collect_expr(high),
            Expr::InList { expr, list, .. } => {
                self.collect_expr(expr) && list.iter().all(|item| self.collect_expr(item))
            }
            Expr::Case {
                operand,
                conditions,
                else_result,
                ..
            } => {
                operand
                    .as_deref()
                    .is_none_or(|expr| self.collect_expr(expr))
                    && conditions.iter().all(|when| {
                        self.collect_expr(&when.condition) && self.collect_expr(&when.result)
                    })
                    && else_result
                        .as_deref()
                        .is_none_or(|expr| self.collect_expr(expr))
            }
            _ => false,
        }
    }

    fn add_spec(&mut self, spec: SimpleAggSpec) {
        if self.index_by_signature.contains_key(&spec.signature) {
            return;
        }
        let idx = self.specs.len();
        self.index_by_signature.insert(spec.signature.clone(), idx);
        self.specs.push(spec);
    }
}

#[derive(Clone)]
struct SimpleAggSpec {
    signature: String,
    kind: SimpleAggKind,
    exprs: Vec<Expr>,
}

#[derive(Clone, Copy)]
enum SimpleAggKind {
    CountStar,
    Count,
    CountDistinct,
    Sum,
    Min,
    Max,
    Avg,
}

enum AggMatch {
    Aggregate(SimpleAggSpec),
    Scalar,
    Unsupported,
}

fn simple_agg_spec(func: &sqlparser::ast::Function) -> AggMatch {
    let name = func.name.to_string().to_ascii_lowercase();
    let FunctionArguments::List(list) = &func.args else {
        return AggMatch::Unsupported;
    };
    if func.filter.is_some() || !list.clauses.is_empty() {
        return AggMatch::Unsupported;
    }
    let distinct = matches!(
        list.duplicate_treatment,
        Some(sqlparser::ast::DuplicateTreatment::Distinct)
    );
    let signature = agg_signature(func);
    match name.as_str() {
        "count" => {
            if list.args.len() == 1
                && matches!(
                    list.args[0],
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard)
                )
            {
                return AggMatch::Aggregate(SimpleAggSpec {
                    signature,
                    kind: SimpleAggKind::CountStar,
                    exprs: Vec::new(),
                });
            }
            let Some(exprs) = function_arg_exprs(&list.args) else {
                return AggMatch::Unsupported;
            };
            AggMatch::Aggregate(SimpleAggSpec {
                signature,
                kind: if distinct {
                    SimpleAggKind::CountDistinct
                } else {
                    SimpleAggKind::Count
                },
                exprs,
            })
        }
        "sum" | "min" | "max" | "avg" if !distinct && list.args.len() == 1 => {
            let Some(exprs) = function_arg_exprs(&list.args) else {
                return AggMatch::Unsupported;
            };
            let kind = match name.as_str() {
                "sum" => SimpleAggKind::Sum,
                "min" => SimpleAggKind::Min,
                "max" => SimpleAggKind::Max,
                "avg" => SimpleAggKind::Avg,
                _ => unreachable!(),
            };
            AggMatch::Aggregate(SimpleAggSpec {
                signature,
                kind,
                exprs,
            })
        }
        _ if is_aggregate_function_name(&name, list.args.len()) => AggMatch::Unsupported,
        _ if crate::udf::is_registered_aggregate(&name) => AggMatch::Unsupported,
        _ => AggMatch::Scalar,
    }
}

fn is_aggregate_function_name(name: &str, arg_count: usize) -> bool {
    matches!(
        name,
        "count"
            | "sum"
            | "avg"
            | "median"
            | "percentile_cont"
            | "group_concat"
            | "string_agg"
            | "total"
            | "json_group_array"
            | "json_group_object"
    ) || (matches!(name, "min" | "max") && arg_count == 1)
}

fn function_exprs(func: &sqlparser::ast::Function) -> Option<Vec<&Expr>> {
    let FunctionArguments::List(list) = &func.args else {
        return None;
    };
    let mut exprs = Vec::with_capacity(list.args.len());
    for arg in &list.args {
        match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => exprs.push(expr),
            _ => return None,
        }
    }
    Some(exprs)
}

fn function_arg_exprs(args: &[FunctionArg]) -> Option<Vec<Expr>> {
    let mut exprs = Vec::with_capacity(args.len());
    for arg in args {
        match arg {
            FunctionArg::Unnamed(FunctionArgExpr::Expr(expr)) => exprs.push(expr.clone()),
            _ => return None,
        }
    }
    Some(exprs)
}

fn agg_signature(func: &sqlparser::ast::Function) -> String {
    format!("{func:?}")
}

struct SimpleGroup {
    first: SqlRow,
    states: Vec<SimpleAggState>,
}

impl SimpleGroup {
    fn new(first: SqlRow, specs: &[SimpleAggSpec]) -> Self {
        Self {
            first,
            states: specs.iter().map(SimpleAggState::new).collect(),
        }
    }

    fn observe(
        &mut self,
        row: &SqlRow,
        specs: &[SimpleAggSpec],
        bindings: &[Option<SqlValue>],
    ) -> Result<()> {
        for (state, spec) in self.states.iter_mut().zip(specs) {
            state.observe(spec, row, bindings)?;
        }
        Ok(())
    }

    fn finalize(&self) -> Vec<SqlValue> {
        self.states.iter().map(SimpleAggState::finalize).collect()
    }
}

struct SimpleAggState {
    kind: SimpleAggKind,
    count: i64,
    total_i: i64,
    total_r: f64,
    saw_real: bool,
    saw_value: bool,
    avg_sum: f64,
    avg_count: i64,
    extremum: Option<SqlValue>,
    distinct_seen: HashSet<Vec<u8>>,
}

impl SimpleAggState {
    fn new(spec: &SimpleAggSpec) -> Self {
        Self {
            kind: spec.kind,
            count: 0,
            total_i: 0,
            total_r: 0.0,
            saw_real: false,
            saw_value: false,
            avg_sum: 0.0,
            avg_count: 0,
            extremum: None,
            distinct_seen: HashSet::new(),
        }
    }

    fn observe(
        &mut self,
        spec: &SimpleAggSpec,
        row: &SqlRow,
        bindings: &[Option<SqlValue>],
    ) -> Result<()> {
        match self.kind {
            SimpleAggKind::CountStar => {
                self.count += 1;
            }
            SimpleAggKind::Count | SimpleAggKind::CountDistinct => {
                let values = eval_agg_args(&spec.exprs, row, bindings)?;
                if values.iter().any(|value| matches!(value, SqlValue::Null)) {
                    return Ok(());
                }
                if matches!(self.kind, SimpleAggKind::CountDistinct) {
                    let key = vec::hash_agg::encode_group_key_bytes(&values)?;
                    if !self.distinct_seen.insert(key) {
                        return Ok(());
                    }
                }
                self.count += 1;
            }
            SimpleAggKind::Sum => {
                let value = eval_single_agg_arg(spec, row, bindings)?;
                self.observe_sum(value)?;
            }
            SimpleAggKind::Avg => {
                let value = eval_single_agg_arg(spec, row, bindings)?;
                self.observe_avg(value)?;
            }
            SimpleAggKind::Min | SimpleAggKind::Max => {
                let value = eval_single_agg_arg(spec, row, bindings)?;
                self.observe_extremum(value);
            }
        }
        Ok(())
    }

    fn observe_sum(&mut self, value: SqlValue) -> Result<()> {
        match value {
            SqlValue::Null => {}
            SqlValue::Integer(v) if !self.saw_real => {
                self.total_i += v;
                self.saw_value = true;
            }
            SqlValue::Integer(v) => {
                self.total_r += v as f64;
                self.saw_value = true;
            }
            SqlValue::Real(v) => {
                if !self.saw_real {
                    self.total_r = self.total_i as f64;
                    self.saw_real = true;
                }
                self.total_r += v;
                self.saw_value = true;
            }
            other => {
                let real = value_to_string(&other)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Error::DatatypeMismatch)?;
                if !self.saw_real {
                    self.total_r = self.total_i as f64;
                    self.saw_real = true;
                }
                self.total_r += real;
                self.saw_value = true;
            }
        }
        Ok(())
    }

    fn observe_avg(&mut self, value: SqlValue) -> Result<()> {
        match value {
            SqlValue::Null => {}
            SqlValue::Integer(v) => {
                self.avg_sum += v as f64;
                self.avg_count += 1;
            }
            SqlValue::Real(v) => {
                self.avg_sum += v;
                self.avg_count += 1;
            }
            other => {
                self.avg_sum += value_to_string(&other)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| Error::DatatypeMismatch)?;
                self.avg_count += 1;
            }
        }
        Ok(())
    }

    fn observe_extremum(&mut self, value: SqlValue) {
        if matches!(value, SqlValue::Null) {
            return;
        }
        self.extremum = match self.extremum.take() {
            None => Some(value),
            Some(current) => {
                let ord = compare_values(&value, &current);
                if (matches!(self.kind, SimpleAggKind::Min) && ord == Ordering::Less)
                    || (matches!(self.kind, SimpleAggKind::Max) && ord == Ordering::Greater)
                {
                    Some(value)
                } else {
                    Some(current)
                }
            }
        };
    }

    fn finalize(&self) -> SqlValue {
        match self.kind {
            SimpleAggKind::CountStar | SimpleAggKind::Count | SimpleAggKind::CountDistinct => {
                SqlValue::Integer(self.count)
            }
            SimpleAggKind::Sum => {
                if !self.saw_value {
                    SqlValue::Null
                } else if self.saw_real {
                    canonicalize(SqlValue::Real(self.total_r))
                } else {
                    SqlValue::Integer(self.total_i)
                }
            }
            SimpleAggKind::Avg => {
                if self.avg_count == 0 {
                    SqlValue::Null
                } else {
                    SqlValue::Real(self.avg_sum / self.avg_count as f64)
                }
            }
            SimpleAggKind::Min | SimpleAggKind::Max => {
                self.extremum.clone().unwrap_or(SqlValue::Null)
            }
        }
    }
}

fn eval_agg_args(
    exprs: &[Expr],
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let ctx = row.context();
    exprs
        .iter()
        .map(|expr| eval_scalar(expr, &ctx, bindings))
        .collect()
}

fn eval_single_agg_arg(
    spec: &SimpleAggSpec,
    row: &SqlRow,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let ctx = row.context();
    eval_scalar(&spec.exprs[0], &ctx, bindings)
}

fn project_simple_group(
    projection: &[SelectItem],
    group: &SimpleGroup,
    values: &[SqlValue],
    specs: &[SimpleAggSpec],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let mut out = Vec::with_capacity(projection.len());
    for item in projection {
        match item {
            SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                out.push(eval_simple_expr(expr, group, values, specs, bindings)?);
            }
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                return Err(Error::UnsupportedSql(
                    "wildcard projection is not supported in simple aggregate".to_owned(),
                ));
            }
        }
    }
    Ok(out)
}

fn eval_simple_expr(
    expr: &Expr,
    group: &SimpleGroup,
    values: &[SqlValue],
    specs: &[SimpleAggSpec],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    if !expr_contains_aggregate(expr) {
        let ctx = group.first.context();
        return eval_scalar(expr, &ctx, bindings);
    }
    match expr {
        Expr::Function(func) => {
            let signature = agg_signature(func);
            if let Some(idx) = specs.iter().position(|spec| spec.signature == signature) {
                return Ok(values[idx].clone());
            }
            let name = func.name.to_string().to_ascii_lowercase();
            let Some(args) = function_exprs(func) else {
                return Err(Error::UnsupportedSql(
                    "unsupported aggregate function call form".to_owned(),
                ));
            };
            let mut arg_values = Vec::with_capacity(args.len());
            for arg in args {
                arg_values.push(eval_simple_expr(arg, group, values, specs, bindings)?);
            }
            crate::exec::expr::json_dispatch::eval_scalar_function_values(&name, arg_values)
        }
        Expr::BinaryOp { left, op, right } => {
            let left_value = eval_simple_expr(left, group, values, specs, bindings)?;
            let right_value = eval_simple_expr(right, group, values, specs, bindings)?;
            eval_simple_binary(left_value, op, right_value)
        }
        Expr::UnaryOp { op, expr } => {
            let value = eval_simple_expr(expr, group, values, specs, bindings)?;
            match op {
                UnaryOperator::Not => match truthy_opt(&value) {
                    Some(v) => Ok(SqlValue::Integer(if !v { 1 } else { 0 })),
                    None => Ok(SqlValue::Null),
                },
                UnaryOperator::Minus => negate(value),
                UnaryOperator::Plus => Ok(value),
                _ => Err(Error::UnsupportedSql(format!(
                    "unsupported unary op {op:?}"
                ))),
            }
        }
        Expr::Nested(expr) => eval_simple_expr(expr, group, values, specs, bindings),
        Expr::Cast {
            expr, data_type, ..
        } => cast_value(
            eval_simple_expr(expr, group, values, specs, bindings)?,
            data_type,
        ),
        Expr::Between {
            expr,
            negated,
            low,
            high,
        } => {
            let value = eval_simple_expr(expr, group, values, specs, bindings)?;
            let low = eval_simple_expr(low, group, values, specs, bindings)?;
            let high = eval_simple_expr(high, group, values, specs, bindings)?;
            if matches!(value, SqlValue::Null)
                || matches!(low, SqlValue::Null)
                || matches!(high, SqlValue::Null)
            {
                Ok(SqlValue::Null)
            } else {
                let mut ok = compare_values(&value, &low) != Ordering::Less
                    && compare_values(&value, &high) != Ordering::Greater;
                if *negated {
                    ok = !ok;
                }
                Ok(SqlValue::Integer(if ok { 1 } else { 0 }))
            }
        }
        Expr::InList {
            expr,
            list,
            negated,
        } => eval_simple_in_list(expr, list, *negated, group, values, specs, bindings),
        Expr::IsNull(expr) => Ok(SqlValue::Integer(
            if matches!(
                eval_simple_expr(expr, group, values, specs, bindings)?,
                SqlValue::Null
            ) {
                1
            } else {
                0
            },
        )),
        Expr::IsNotNull(expr) => Ok(SqlValue::Integer(
            if !matches!(
                eval_simple_expr(expr, group, values, specs, bindings)?,
                SqlValue::Null
            ) {
                1
            } else {
                0
            },
        )),
        Expr::IsTrue(expr) => Ok(sql_truth_result(eval_simple_expr(
            expr, group, values, specs, bindings,
        )?)),
        Expr::IsNotTrue(expr) => Ok(sql_truth_result_not(eval_simple_expr(
            expr, group, values, specs, bindings,
        )?)),
        Expr::IsFalse(expr) => Ok(sql_false_result(eval_simple_expr(
            expr, group, values, specs, bindings,
        )?)),
        Expr::IsNotFalse(expr) => Ok(sql_false_result_not(eval_simple_expr(
            expr, group, values, specs, bindings,
        )?)),
        Expr::IsUnknown(expr) => Ok(SqlValue::Integer(
            if matches!(
                eval_simple_expr(expr, group, values, specs, bindings)?,
                SqlValue::Null
            ) {
                1
            } else {
                0
            },
        )),
        Expr::IsNotUnknown(expr) => Ok(SqlValue::Integer(
            if !matches!(
                eval_simple_expr(expr, group, values, specs, bindings)?,
                SqlValue::Null
            ) {
                1
            } else {
                0
            },
        )),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            let mut evaluator = SimpleCaseEvaluator {
                group,
                values,
                specs,
                bindings,
            };
            eval_case(
                operand.as_deref(),
                conditions,
                else_result.as_deref(),
                &mut evaluator,
            )
        }
        _ => Err(Error::UnsupportedSql(
            "aggregate expressions in this query are not supported".to_owned(),
        )),
    }
}

fn eval_simple_binary(
    left_value: SqlValue,
    op: &BinaryOperator,
    right_value: SqlValue,
) -> Result<SqlValue> {
    Ok(match op {
        BinaryOperator::And => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(false), _) | (_, Some(false)) => SqlValue::Integer(0),
            (Some(true), Some(true)) => SqlValue::Integer(1),
            _ => SqlValue::Null,
        },
        BinaryOperator::Or => match (truthy_opt(&left_value), truthy_opt(&right_value)) {
            (Some(true), _) | (_, Some(true)) => SqlValue::Integer(1),
            (Some(false), Some(false)) => SqlValue::Integer(0),
            _ => SqlValue::Null,
        },
        BinaryOperator::Plus => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_add(b)),
            |a, b| Some(a + b),
        )?,
        BinaryOperator::Minus => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_sub(b)),
            |a, b| Some(a - b),
        )?,
        BinaryOperator::Multiply => arithmetic(
            left_value,
            right_value,
            |a, b| Some(a.wrapping_mul(b)),
            |a, b| Some(a * b),
        )?,
        BinaryOperator::Divide => arithmetic(
            left_value,
            right_value,
            |a, b| if b == 0 { None } else { a.checked_div(b) },
            |a, b| if b == 0.0 { None } else { Some(a / b) },
        )?,
        BinaryOperator::Modulo => arithmetic(
            left_value,
            right_value,
            |a, b| if b == 0 { None } else { a.checked_rem(b) },
            |a, b| if b == 0.0 { None } else { Some(a % b) },
        )?,
        BinaryOperator::Eq => {
            compare_binary(left_value, right_value, |ord| ord == Ordering::Equal)?
        }
        BinaryOperator::NotEq | BinaryOperator::Spaceship => {
            compare_binary(left_value, right_value, |ord| ord != Ordering::Equal)?
        }
        BinaryOperator::Gt => {
            compare_binary(left_value, right_value, |ord| ord == Ordering::Greater)?
        }
        BinaryOperator::GtEq => {
            compare_binary(left_value, right_value, |ord| ord != Ordering::Less)?
        }
        BinaryOperator::Lt => compare_binary(left_value, right_value, |ord| ord == Ordering::Less)?,
        BinaryOperator::LtEq => {
            compare_binary(left_value, right_value, |ord| ord != Ordering::Greater)?
        }
        BinaryOperator::StringConcat => {
            if matches!(left_value, SqlValue::Null) || matches!(right_value, SqlValue::Null) {
                SqlValue::Null
            } else {
                SqlValue::Text(Arc::from(format!(
                    "{}{}",
                    value_to_string(&left_value),
                    value_to_string(&right_value)
                )))
            }
        }
        other => {
            return Err(Error::UnsupportedSql(format!(
                "unsupported binary op {other:?}"
            )));
        }
    })
}

fn eval_simple_in_list(
    expr: &Expr,
    list: &[Expr],
    negated: bool,
    group: &SimpleGroup,
    values: &[SqlValue],
    specs: &[SimpleAggSpec],
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let value = eval_simple_expr(expr, group, values, specs, bindings)?;
    if matches!(value, SqlValue::Null) {
        return Ok(SqlValue::Null);
    }
    let mut found = false;
    let mut saw_null = false;
    for item in list {
        let candidate = eval_simple_expr(item, group, values, specs, bindings)?;
        match candidate {
            SqlValue::Null => saw_null = true,
            _ if compare_values(&value, &candidate) == Ordering::Equal => {
                found = true;
                break;
            }
            _ => {}
        }
    }
    let mut ok = found;
    if negated {
        ok = !ok;
    }
    if !ok && saw_null {
        Ok(SqlValue::Null)
    } else {
        Ok(SqlValue::Integer(if ok { 1 } else { 0 }))
    }
}

struct SimpleCaseEvaluator<'a> {
    group: &'a SimpleGroup,
    values: &'a [SqlValue],
    specs: &'a [SimpleAggSpec],
    bindings: &'a [Option<SqlValue>],
}

impl CaseEvaluator for SimpleCaseEvaluator<'_> {
    fn eval_case_expr(&mut self, expr: &Expr) -> Result<SqlValue> {
        eval_simple_expr(expr, self.group, self.values, self.specs, self.bindings)
    }
}

fn eval_order_keys(
    projection: &[SelectItem],
    order_by: &[OrderByExpr],
    projected: &[SqlValue],
    group: &SimpleGroup,
    values: &[SqlValue],
    specs: &[SimpleAggSpec],
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    let mut keys = Vec::with_capacity(order_by.len());
    for order in order_by {
        if let Some(idx) = order_projection_index(&order.expr, projection)
            && let Some(value) = projected.get(idx)
        {
            keys.push(value.clone());
            continue;
        }
        keys.push(eval_simple_expr(
            &order.expr,
            group,
            values,
            specs,
            bindings,
        )?);
    }
    Ok(keys)
}

fn sort_projected_with_keys(
    projected: &mut [Vec<SqlValue>],
    order_keys: &mut [Vec<SqlValue>],
    order_by: &[OrderByExpr],
) {
    if projected.len() != order_keys.len() {
        projected.sort_by(|left, right| compare_rows(left, right));
        return;
    }
    let mut indices: Vec<usize> = (0..projected.len()).collect();
    indices.sort_by(|&left, &right| {
        for (idx, order) in order_by.iter().enumerate() {
            let mut ord = compare_values(&order_keys[left][idx], &order_keys[right][idx]);
            if matches!(order.options.asc, Some(false)) {
                ord = ord.reverse();
            }
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    });
    let sorted_projected: Vec<Vec<SqlValue>> =
        indices.iter().map(|idx| projected[*idx].clone()).collect();
    let sorted_keys: Vec<Vec<SqlValue>> =
        indices.iter().map(|idx| order_keys[*idx].clone()).collect();
    projected.clone_from_slice(&sorted_projected);
    order_keys.clone_from_slice(&sorted_keys);
}

fn order_projection_index(expr: &Expr, projection: &[SelectItem]) -> Option<usize> {
    if let Expr::Value(value) = expr
        && let Value::Number(number, _) = &value.value
        && let Ok(pos) = number.parse::<usize>()
        && pos > 0
        && pos <= projection.len()
    {
        return Some(pos - 1);
    }
    let target = match expr {
        Expr::Identifier(ident) => ident.value.as_str(),
        Expr::CompoundIdentifier(parts) if parts.len() == 1 => parts[0].value.as_str(),
        _ => return None,
    };
    for (idx, item) in projection.iter().enumerate() {
        match item {
            SelectItem::ExprWithAlias { alias, .. } if alias.value.eq_ignore_ascii_case(target) => {
                return Some(idx);
            }
            SelectItem::UnnamedExpr(expr) if matches_simple_identifier(expr, target) => {
                return Some(idx);
            }
            _ => {}
        }
    }
    None
}

fn matches_simple_identifier(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Identifier(ident) => ident.value.eq_ignore_ascii_case(target),
        Expr::CompoundIdentifier(parts) => parts
            .last()
            .is_some_and(|part| part.value.eq_ignore_ascii_case(target)),
        Expr::Nested(inner) => matches_simple_identifier(inner, target),
        _ => false,
    }
}

fn projection_has_wildcard(projection: &[SelectItem]) -> bool {
    projection.iter().any(|item| {
        matches!(
            item,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
        )
    })
}

fn projection_is_aggregate_only(projection: &[SelectItem]) -> bool {
    !projection.is_empty()
        && projection.iter().all(|item| match item {
            SelectItem::UnnamedExpr(expr) | SelectItem::ExprWithAlias { expr, .. } => {
                expr_contains_aggregate(expr)
            }
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
        })
}
