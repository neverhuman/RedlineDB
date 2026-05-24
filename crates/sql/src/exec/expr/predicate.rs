use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct SubqueryCacheKey {
    ast_addr: usize,
    schema_epoch: u64,
    stats_epoch: u64,
    optimizer_hash: u64,
}

thread_local! {
    static SUBQUERY_TEMPLATE_CACHE: RefCell<HashMap<SubqueryCacheKey, PreparedTemplate>> =
        RefCell::new(HashMap::new());
    static UNCORRELATED_IN_ROWS_CACHE: RefCell<HashMap<SubqueryCacheKey, InRowsCacheEntry>> =
        RefCell::new(HashMap::new());
    static CORRELATED_EXISTS_CACHE: RefCell<HashMap<SubqueryCacheKey, ExistsCacheEntry>> =
        RefCell::new(HashMap::new());
}

pub(crate) fn clear_subquery_template_cache() {
    SUBQUERY_TEMPLATE_CACHE.with(|cache| cache.borrow_mut().clear());
    UNCORRELATED_IN_ROWS_CACHE.with(|cache| cache.borrow_mut().clear());
    CORRELATED_EXISTS_CACHE.with(|cache| cache.borrow_mut().clear());
}

pub(crate) fn truthy_opt(value: &SqlValue) -> Option<bool> {
    match value {
        SqlValue::Null => None,
        _ => Some(is_truthy(value)),
    }
}

pub(crate) trait CaseEvaluator {
    fn eval_case_expr(&mut self, expr: &Expr) -> Result<SqlValue>;
}

pub(crate) fn eval_case<E>(
    operand: Option<&Expr>,
    conditions: &[sqlparser::ast::CaseWhen],
    else_result: Option<&Expr>,
    evaluator: &mut E,
) -> Result<SqlValue>
where
    E: CaseEvaluator,
{
    if let Some(operand) = operand {
        let operand = evaluator.eval_case_expr(operand)?;
        if matches!(operand, SqlValue::Null) {
            return match else_result {
                Some(expr) => evaluator.eval_case_expr(expr),
                None => Ok(SqlValue::Null),
            };
        }
        for when in conditions {
            let condition = evaluator.eval_case_expr(&when.condition)?;
            if matches!(condition, SqlValue::Null) {
                continue;
            }
            if compare_values(&operand, &condition) == Ordering::Equal {
                return evaluator.eval_case_expr(&when.result);
            }
        }
    } else {
        for when in conditions {
            let condition = evaluator.eval_case_expr(&when.condition)?;
            if !matches!(condition, SqlValue::Null) && is_truthy(&condition) {
                return evaluator.eval_case_expr(&when.result);
            }
        }
    }
    match else_result {
        Some(expr) => evaluator.eval_case_expr(expr),
        None => Ok(SqlValue::Null),
    }
}

pub(crate) fn eval_subquery_value(
    subquery: &sqlparser::ast::Query,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let rows = evaluate_subquery_rows(subquery, row, bindings)?;
    match rows.as_slice() {
        [] => Ok(SqlValue::Null),
        [row] if row.len() == 1 => Ok(row[0].clone()),
        [row] if row.is_empty() => Ok(SqlValue::Null),
        _ => Err(Error::UnsupportedSql(
            "scalar subquery must return exactly one row and one column".to_owned(),
        )),
    }
}

fn bind_subquery(conn: &Connection, subquery: &sqlparser::ast::Query) -> Result<PreparedTemplate> {
    let key = subquery_cache_key(conn, subquery);
    if let Some(template) = SUBQUERY_TEMPLATE_CACHE.with(|cache| cache.borrow().get(&key).cloned())
    {
        return Ok(template);
    }
    let schema =
        current_tx_schema_snapshot(conn).unwrap_or_else(|| conn.engine().schema_snapshot());
    let template = crate::parser::bind_query(
        conn,
        schema,
        conn.schema_epoch(),
        "<subquery>",
        subquery.clone(),
    )?;
    SUBQUERY_TEMPLATE_CACHE.with(|cache| {
        cache.borrow_mut().insert(key, template.clone());
    });
    Ok(template)
}

fn subquery_cache_key(conn: &Connection, subquery: &sqlparser::ast::Query) -> SubqueryCacheKey {
    SubqueryCacheKey {
        ast_addr: subquery as *const sqlparser::ast::Query as usize,
        schema_epoch: conn.schema_epoch().0,
        stats_epoch: conn.stats_epoch().0,
        optimizer_hash: conn.optimizer_hash(),
    }
}

/// Evaluate a subquery, pushing the caller's row onto the correlated-scope
/// stack so qualified references (`outer.col`) resolve through
/// `lookup_correlated`. The row snapshot is dropped automatically once
/// the subquery returns.
pub(crate) fn evaluate_subquery_rows(
    subquery: &sqlparser::ast::Query,
    outer_row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<Vec<SqlValue>>> {
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    let owned = outer_row.to_owned_row();
    crate::exec::with_outer_row(owned, || {
        materialize_prepared_rows(conn, &template, bindings)
    })
}

pub(crate) fn evaluate_subquery_exists(
    subquery: &sqlparser::ast::Query,
    outer_row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<bool> {
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    let key = subquery_cache_key(conn, subquery);
    if let Some(exists) = evaluate_fast_exists(conn, key, &template, outer_row, bindings)? {
        return Ok(exists);
    }
    let owned = outer_row.to_owned_row();
    let rows = crate::exec::with_outer_row(owned, || {
        materialize_prepared_rows_limited(conn, &template, bindings, Some(1))
    })?;
    Ok(!rows.is_empty())
}

fn row_values_for_expr(
    expr: &Expr,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<Vec<SqlValue>> {
    match expr {
        Expr::Tuple(exprs) => exprs
            .iter()
            .map(|expr| eval_scalar(expr, row, bindings))
            .collect(),
        Expr::Nested(inner) => row_values_for_expr(inner, row, bindings),
        _ => Ok(vec![eval_scalar(expr, row, bindings)?]),
    }
}

fn row_eq(left: &[SqlValue], right: &[SqlValue]) -> Result<Option<bool>> {
    if left.len() != right.len() {
        return Err(Error::UnsupportedSql(format!(
            "row value arity mismatch: {} vs {}",
            left.len(),
            right.len()
        )));
    }
    for (l, r) in left.iter().zip(right.iter()) {
        if matches!(l, SqlValue::Null) || matches!(r, SqlValue::Null) {
            return Ok(None);
        }
        match compare_values(l, r) {
            Ordering::Equal => {}
            _ => return Ok(Some(false)),
        }
    }
    Ok(Some(true))
}

pub(crate) fn in_list_result(
    expr: &Expr,
    list: &[Expr],
    negated: bool,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let value = row_values_for_expr(expr, row, bindings)?;
    if value.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let mut found = false;
    let mut saw_null = false;
    for item in list {
        let candidate = row_values_for_expr(item, row, bindings)?;
        match row_eq(&value, &candidate)? {
            Some(true) => {
                found = true;
                break;
            }
            Some(false) => {}
            None => saw_null = true,
        }
    }
    finish_in_result(found, saw_null, negated)
}

pub(crate) fn in_subquery_result(
    expr: &Expr,
    subquery: &sqlparser::ast::Query,
    negated: bool,
    row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<SqlValue> {
    let value = row_values_for_expr(expr, row, bindings)?;
    if value.iter().any(|v| matches!(v, SqlValue::Null)) {
        return Ok(SqlValue::Null);
    }
    let Some(conn) = current_connection() else {
        return Err(Error::TransactionState(
            "subquery evaluation requires an active connection",
        ));
    };
    let template = bind_subquery(conn, subquery)?;
    if template.output_columns.len() != value.len() {
        return Err(Error::UnsupportedSql(
            "IN subquery must return the same number of columns as the row value".to_owned(),
        ));
    }
    let key = subquery_cache_key(conn, subquery);
    if let Some(entry) = UNCORRELATED_IN_ROWS_CACHE.with(|cache| cache.borrow().get(&key).cloned())
    {
        if let InRowsCacheEntry::Rows(rows) = entry {
            return finish_in_rows(&value, rows.iter(), negated);
        }
    } else {
        match materialize_prepared_rows(conn, &template, bindings) {
            Ok(rows) => {
                UNCORRELATED_IN_ROWS_CACHE.with(|cache| {
                    cache
                        .borrow_mut()
                        .insert(key, InRowsCacheEntry::Rows(Arc::from(rows)));
                });
                if let Some(InRowsCacheEntry::Rows(rows)) =
                    UNCORRELATED_IN_ROWS_CACHE.with(|cache| cache.borrow().get(&key).cloned())
                {
                    return finish_in_rows(&value, rows.iter(), negated);
                }
            }
            Err(Error::UnknownColumn(_)) => {
                UNCORRELATED_IN_ROWS_CACHE.with(|cache| {
                    cache.borrow_mut().insert(key, InRowsCacheEntry::Correlated);
                });
            }
            Err(err) => return Err(err),
        }
    }
    let owned = row.to_owned_row();
    let rows = crate::exec::with_outer_row(owned, || {
        materialize_prepared_rows(conn, &template, bindings)
    })?;
    finish_in_rows(&value, rows.iter(), negated)
}

#[derive(Clone)]
enum InRowsCacheEntry {
    Rows(Arc<[Vec<SqlValue>]>),
    Correlated,
}

#[derive(Clone)]
enum ExistsCacheEntry {
    Plan(FastExistsPlan),
    Unsupported,
}

#[derive(Clone)]
struct FastExistsPlan {
    table: Arc<TableDef>,
    predicate: Option<FastExistsExpr>,
}

#[derive(Clone)]
enum FastExistsExpr {
    And(Box<FastExistsExpr>, Box<FastExistsExpr>),
    Comparison {
        left: FastExistsValue,
        op: FastCompareOp,
        right: FastExistsValue,
    },
}

impl FastExistsExpr {
    fn evaluate(
        &self,
        inner: &TableRow,
        outer: &RowContext<'_>,
        bindings: &[Option<SqlValue>],
    ) -> Result<bool> {
        match self {
            Self::And(left, right) => {
                if !left.evaluate(inner, outer, bindings)? {
                    return Ok(false);
                }
                right.evaluate(inner, outer, bindings)
            }
            Self::Comparison { left, op, right } => {
                let left = left.evaluate(inner, outer, bindings)?;
                let right = right.evaluate(inner, outer, bindings)?;
                if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
                    return Ok(false);
                }
                Ok(op.accept(compare_values(&left, &right)))
            }
        }
    }
}

#[derive(Clone)]
enum FastExistsValue {
    Inner(TableValueRef),
    Literal(SqlValue),
    Outer(Expr),
}

impl FastExistsValue {
    fn evaluate(
        &self,
        inner: &TableRow,
        outer: &RowContext<'_>,
        bindings: &[Option<SqlValue>],
    ) -> Result<SqlValue> {
        match self {
            Self::Inner(value) => Ok(value.sql_value(inner).unwrap_or(SqlValue::Null)),
            Self::Literal(value) => Ok(value.clone()),
            Self::Outer(expr) => eval_scalar(expr, outer, bindings),
        }
    }
}

fn evaluate_fast_exists(
    conn: &Connection,
    key: SubqueryCacheKey,
    template: &PreparedTemplate,
    outer_row: &RowContext<'_>,
    bindings: &[Option<SqlValue>],
) -> Result<Option<bool>> {
    let entry = CORRELATED_EXISTS_CACHE.with(|cache| cache.borrow().get(&key).cloned());
    let plan = match entry {
        Some(ExistsCacheEntry::Plan(plan)) => plan,
        Some(ExistsCacheEntry::Unsupported) => return Ok(None),
        None => {
            let entry = compile_fast_exists_plan(template)
                .map(ExistsCacheEntry::Plan)
                .unwrap_or(ExistsCacheEntry::Unsupported);
            CORRELATED_EXISTS_CACHE.with(|cache| {
                cache.borrow_mut().insert(key, entry.clone());
            });
            match entry {
                ExistsCacheEntry::Plan(plan) => plan,
                ExistsCacheEntry::Unsupported => return Ok(None),
            }
        }
    };

    match crate::udf::authorize_table_access(crate::udf::AUTH_SELECT, &plan.table.name) {
        crate::udf::AuthorizerDecision::Allow => {}
        crate::udf::AuthorizerDecision::Deny => return Err(Error::NotAuthorized),
        crate::udf::AuthorizerDecision::Ignore => return Ok(Some(false)),
    }

    let Some(tx_ptr) = current_tx() else {
        return Ok(None);
    };
    // SAFETY: current_tx installs this pointer for the synchronous statement
    // scope; nested SELECT execution already reuses it through the same guard.
    let tx = unsafe { &mut *tx_ptr };
    let mut rowids = conn.engine().relation_rowids(plan.table.relation_id)?;
    rowids.sort();
    for rowid in rowids {
        let Some(row) = load_table_row_by_rowid(conn.engine(), tx, &plan.table, rowid)? else {
            continue;
        };
        let matches = match &plan.predicate {
            Some(predicate) => predicate.evaluate(&row, outer_row, bindings)?,
            None => true,
        };
        if matches {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

fn compile_fast_exists_plan(template: &PreparedTemplate) -> Option<FastExistsPlan> {
    let PreparedKind::Select(plan) = &template.kind else {
        return None;
    };
    if plan.distinct
        || select_requires_aggregation(plan)
        || !plan.group_by.is_empty()
        || plan.having.is_some()
        || !plan.order_by.is_empty()
        || plan.limit.is_some()
        || plan.offset.is_some()
    {
        return None;
    }
    let SelectSource::Table(table) = &plan.source else {
        return None;
    };
    let predicate = match &plan.selection {
        Some(expr) => Some(compile_fast_exists_expr(table, expr)?),
        None => None,
    };
    Some(FastExistsPlan {
        table: Arc::clone(table),
        predicate,
    })
}

fn compile_fast_exists_expr(table: &TableDef, expr: &Expr) -> Option<FastExistsExpr> {
    match strip_nested(expr) {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => Some(FastExistsExpr::And(
            Box::new(compile_fast_exists_expr(table, left)?),
            Box::new(compile_fast_exists_expr(table, right)?),
        )),
        Expr::BinaryOp { left, op, right } => {
            let op = FastCompareOp::from_binary(op)?;
            Some(FastExistsExpr::Comparison {
                left: compile_fast_exists_value(table, left)?,
                op,
                right: compile_fast_exists_value(table, right)?,
            })
        }
        _ => None,
    }
}

fn compile_fast_exists_value(table: &TableDef, expr: &Expr) -> Option<FastExistsValue> {
    if let Some(inner) = compile_table_value_ref(table, expr) {
        return Some(FastExistsValue::Inner(inner));
    }
    if let Some(value) = literal_value(expr, &[]) {
        return Some(FastExistsValue::Literal(value));
    }
    if is_outer_scalar_value(expr) {
        return Some(FastExistsValue::Outer(strip_nested(expr).clone()));
    }
    None
}

fn is_outer_scalar_value(expr: &Expr) -> bool {
    match strip_nested(expr) {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
        Expr::Value(value) => crate::parser::bind::as_bind_name(&value.value).is_some(),
        Expr::UnaryOp {
            op: UnaryOperator::Plus | UnaryOperator::Minus,
            expr,
        } => is_outer_scalar_value(expr),
        _ => false,
    }
}

fn finish_in_rows<'a>(
    value: &[SqlValue],
    rows: impl IntoIterator<Item = &'a Vec<SqlValue>>,
    negated: bool,
) -> Result<SqlValue> {
    let mut found = false;
    let mut saw_null = false;
    for row in rows {
        match row_eq(value, row)? {
            Some(true) => {
                found = true;
                break;
            }
            Some(false) => {}
            None => saw_null = true,
        }
    }
    finish_in_result(found, saw_null, negated)
}

fn finish_in_result(found: bool, saw_null: bool, negated: bool) -> Result<SqlValue> {
    let base_in: Option<bool> = if found {
        Some(true)
    } else if saw_null {
        None
    } else {
        Some(false)
    };
    Ok(match (base_in, negated) {
        (Some(b), false) => SqlValue::Integer(if b { 1 } else { 0 }),
        (Some(b), true) => SqlValue::Integer(if !b { 1 } else { 0 }),
        (None, _) => SqlValue::Null,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct TablePredicate {
    expr: FastPredicateExpr,
}

impl TablePredicate {
    fn evaluate(&self, row: &TableRow) -> Option<bool> {
        self.expr.evaluate(row)
    }
}

#[derive(Debug, Clone)]
enum FastPredicateExpr {
    And(Box<FastPredicateExpr>, Box<FastPredicateExpr>),
    Comparison {
        left: FastPredicateValue,
        op: FastCompareOp,
        right: FastPredicateValue,
    },
}

impl FastPredicateExpr {
    fn evaluate(&self, row: &TableRow) -> Option<bool> {
        match self {
            Self::And(left, right) => {
                let left = left.evaluate(row)?;
                if !left {
                    return Some(false);
                }
                Some(right.evaluate(row)?)
            }
            Self::Comparison { left, op, right } => {
                let left = left.evaluate(row)?;
                let right = right.evaluate(row)?;
                if matches!(left, SqlValue::Null) || matches!(right, SqlValue::Null) {
                    return Some(false);
                }
                Some(op.accept(compare_values(&left, &right)))
            }
        }
    }
}

#[derive(Debug, Clone)]
enum FastPredicateValue {
    Column(TableValueRef),
    Literal(SqlValue),
    Modulo { value: TableValueRef, modulus: i64 },
}

impl FastPredicateValue {
    fn evaluate(&self, row: &TableRow) -> Option<SqlValue> {
        match self {
            Self::Column(value) => value.sql_value(row),
            Self::Literal(value) => Some(value.clone()),
            Self::Modulo { value, modulus } => {
                if *modulus == 0 {
                    return Some(SqlValue::Null);
                }
                match value.sql_value(row)? {
                    SqlValue::Null => Some(SqlValue::Null),
                    SqlValue::Integer(v) => Some(SqlValue::Integer(v % *modulus)),
                    _ => None,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TableValueRef {
    Rowid,
    Column(usize),
}

impl TableValueRef {
    fn sql_value(self, row: &TableRow) -> Option<SqlValue> {
        match self {
            Self::Rowid => Some(SqlValue::Integer(row.rowid.0 as i64)),
            Self::Column(ordinal) => row.values.get(ordinal).cloned(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FastCompareOp {
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
}

impl FastCompareOp {
    fn from_binary(op: &BinaryOperator) -> Option<Self> {
        match op {
            BinaryOperator::Eq => Some(Self::Eq),
            BinaryOperator::NotEq => Some(Self::NotEq),
            BinaryOperator::Gt => Some(Self::Gt),
            BinaryOperator::GtEq => Some(Self::GtEq),
            BinaryOperator::Lt => Some(Self::Lt),
            BinaryOperator::LtEq => Some(Self::LtEq),
            _ => None,
        }
    }

    fn accept(self, ordering: Ordering) -> bool {
        match self {
            Self::Eq => ordering == Ordering::Equal,
            Self::NotEq => ordering != Ordering::Equal,
            Self::Gt => ordering == Ordering::Greater,
            Self::GtEq => ordering != Ordering::Less,
            Self::Lt => ordering == Ordering::Less,
            Self::LtEq => ordering != Ordering::Greater,
        }
    }
}

pub(crate) fn compile_table_predicate(
    table: &TableDef,
    selection: &Option<Expr>,
    bindings: &[Option<SqlValue>],
) -> Option<TablePredicate> {
    selection.as_ref().and_then(|expr| {
        compile_predicate_expr(table, expr, bindings).map(|expr| TablePredicate { expr })
    })
}

pub(crate) fn selection_passes_table(
    selection: &Option<Expr>,
    compiled: Option<&TablePredicate>,
    row: &TableRow,
    bindings: &[Option<SqlValue>],
) -> Result<bool> {
    if selection.is_none() {
        return Ok(true);
    }
    if let Some(compiled) = compiled
        && let Some(value) = compiled.evaluate(row)
    {
        return Ok(value);
    }
    selection_passes(selection, &SqlRow::Table(row.clone()), bindings)
}

fn compile_predicate_expr(
    table: &TableDef,
    expr: &Expr,
    bindings: &[Option<SqlValue>],
) -> Option<FastPredicateExpr> {
    match strip_nested(expr) {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => Some(FastPredicateExpr::And(
            Box::new(compile_predicate_expr(table, left, bindings)?),
            Box::new(compile_predicate_expr(table, right, bindings)?),
        )),
        Expr::BinaryOp { left, op, right } => {
            let op = FastCompareOp::from_binary(op)?;
            Some(FastPredicateExpr::Comparison {
                left: compile_predicate_value(table, left, bindings)?,
                op,
                right: compile_predicate_value(table, right, bindings)?,
            })
        }
        _ => None,
    }
}

fn compile_predicate_value(
    table: &TableDef,
    expr: &Expr,
    bindings: &[Option<SqlValue>],
) -> Option<FastPredicateValue> {
    match strip_nested(expr) {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::Modulo,
            right,
        } => {
            let value = compile_table_value_ref(table, left)?;
            let SqlValue::Integer(modulus) = literal_value(right, bindings)? else {
                return None;
            };
            Some(FastPredicateValue::Modulo { value, modulus })
        }
        expr => compile_table_value_ref(table, expr)
            .map(FastPredicateValue::Column)
            .or_else(|| literal_value(expr, bindings).map(FastPredicateValue::Literal)),
    }
}

fn compile_table_value_ref(table: &TableDef, expr: &Expr) -> Option<TableValueRef> {
    let name = match strip_nested(expr) {
        Expr::Identifier(ident) => ident.value.as_str(),
        Expr::CompoundIdentifier(parts) if parts.len() == 2 => {
            let qualifier = parts.first()?.value.as_str();
            if !table.name.as_ref().eq_ignore_ascii_case(qualifier)
                && !table.folded.as_ref().eq_ignore_ascii_case(qualifier)
            {
                return None;
            }
            parts.last()?.value.as_str()
        }
        _ => return None,
    };
    if table.is_public_rowid_name(name) {
        return Some(TableValueRef::Rowid);
    }
    table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
        .map(TableValueRef::Column)
}

fn literal_value(expr: &Expr, bindings: &[Option<SqlValue>]) -> Option<SqlValue> {
    if let Expr::Value(v) = expr
        && let Some(name) = crate::parser::bind::as_bind_name(&v.value)
    {
        return crate::parser::bind::resolve_positional(name, bindings);
    }
    match strip_nested(expr) {
        Expr::Value(v) => match &v.value {
            Value::Null => Some(SqlValue::Null),
            Value::Boolean(value) => Some(SqlValue::Integer(if *value { 1 } else { 0 })),
            Value::Number(value, _) => parse_number(value).ok(),
            Value::SingleQuotedString(value)
            | Value::DoubleQuotedString(value)
            | Value::EscapedStringLiteral(value)
            | Value::TripleSingleQuotedString(value)
            | Value::TripleDoubleQuotedString(value)
            | Value::UnicodeStringLiteral(value)
            | Value::SingleQuotedRawStringLiteral(value)
            | Value::DoubleQuotedRawStringLiteral(value)
            | Value::TripleSingleQuotedRawStringLiteral(value)
            | Value::TripleDoubleQuotedRawStringLiteral(value) => {
                Some(SqlValue::Text(Arc::from(value.as_str())))
            }
            _ => None,
        },
        Expr::UnaryOp { op, expr } => {
            let value = literal_value(expr, bindings)?;
            match op {
                UnaryOperator::Minus => match value {
                    SqlValue::Integer(value) => Some(SqlValue::Integer(-value)),
                    SqlValue::Real(value) => Some(SqlValue::Real(-value)),
                    _ => None,
                },
                UnaryOperator::Plus => Some(value),
                _ => None,
            }
        }
        _ => None,
    }
}

fn strip_nested(expr: &Expr) -> &Expr {
    let mut current = expr;
    while let Expr::Nested(inner) = current {
        current = inner;
    }
    current
}
