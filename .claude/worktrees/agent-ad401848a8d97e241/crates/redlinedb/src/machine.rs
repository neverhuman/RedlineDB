use std::fmt::Write as _;
use std::sync::Arc;

use redlinedb_kernel::catalog::{SchemaSnapshot, TableDef, TableId};

use crate::error::{Error, ErrorCode, Result};
use crate::{Database, Prepared};

#[derive(Clone, Debug, PartialEq)]
pub enum QuerySpec {
    Select(SelectSpec),
    Insert(InsertSpec),
    Update(UpdateSpec),
    Delete(DeleteSpec),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectSpec {
    pub from: TableRef,
    pub columns: Vec<ColumnRef>,
    pub filter: Option<ExprSpec>,
    pub order_by: Vec<OrderSpec>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub max_cost: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRef {
    pub table_id: u64,
    pub schema_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnRef {
    pub table_id: u64,
    pub column_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprSpec {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
    Param(usize),
    Column(ColumnRef),
    Nested(Box<ExprSpec>),
    Unary {
        op: UnaryOp,
        expr: Box<ExprSpec>,
    },
    Binary {
        left: Box<ExprSpec>,
        op: BinaryOp,
        right: Box<ExprSpec>,
    },
    Function {
        name: String,
        args: Vec<ExprSpec>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
    Positive,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Like,
    Is,
    IsNot,
    Concat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrderSpec {
    pub expr: ExprSpec,
    pub descending: bool,
    pub nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InsertSpec {
    pub into: TableRef,
    pub columns: Vec<ColumnRef>,
    pub values: Vec<Vec<ExprSpec>>,
    pub default_values: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateSpec {
    pub table: TableRef,
    pub assignments: Vec<(ColumnRef, ExprSpec)>,
    pub filter: Option<ExprSpec>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteSpec {
    pub from: TableRef,
    pub filter: Option<ExprSpec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaHandle {
    pub schema_epoch: u64,
}

impl Database {
    pub fn schema_handle(&self) -> Result<SchemaHandle> {
        Ok(SchemaHandle {
            schema_epoch: self.inner.db.schema_epoch().0,
        })
    }

    pub fn prepare_query_spec(&self, spec: &QuerySpec) -> Result<Prepared> {
        let snapshot = self.inner.db.schema_snapshot();
        let sql = match spec {
            QuerySpec::Select(select) => render_select(&snapshot, select)?,
            QuerySpec::Insert(insert) => render_insert(&snapshot, insert)?,
            QuerySpec::Update(update) => render_update(&snapshot, update)?,
            QuerySpec::Delete(delete) => render_delete(&snapshot, delete)?,
        };
        self.prepare(&sql)
    }
}

fn render_select(snapshot: &SchemaSnapshot, spec: &SelectSpec) -> Result<String> {
    if spec.max_cost.is_some() {
        return Err(Error::unsupported("max_cost hints are not supported"));
    }
    let table = resolve_table(snapshot, spec.from)?;
    let mut sql = String::new();
    sql.push_str("SELECT ");
    if spec.columns.is_empty() {
        sql.push('*');
    } else {
        for (index, column) in spec.columns.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&render_column_ref(snapshot, &table, *column)?);
        }
    }
    sql.push_str(" FROM ");
    sql.push_str(&render_table_name(&table));

    if let Some(filter) = &spec.filter {
        sql.push_str(" WHERE ");
        sql.push_str(&render_expr(snapshot, &table, filter)?);
    }

    if !spec.order_by.is_empty() {
        sql.push_str(" ORDER BY ");
        for (index, order) in spec.order_by.iter().enumerate() {
            if index > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&render_expr(snapshot, &table, &order.expr)?);
            if order.descending {
                sql.push_str(" DESC");
            } else {
                sql.push_str(" ASC");
            }
            match order.nulls_first {
                Some(true) => sql.push_str(" NULLS FIRST"),
                Some(false) => sql.push_str(" NULLS LAST"),
                None => {}
            }
        }
    }

    if let Some(limit) = spec.limit {
        write!(sql, " LIMIT {limit}").expect("write to string");
    }
    if let Some(offset) = spec.offset {
        write!(sql, " OFFSET {offset}").expect("write to string");
    }

    Ok(sql)
}

fn render_insert(snapshot: &SchemaSnapshot, spec: &InsertSpec) -> Result<String> {
    let table = resolve_table(snapshot, spec.into)?;
    if spec.default_values && !spec.values.is_empty() {
        return Err(Error::new(
            ErrorCode::Misuse,
            "default_values cannot be combined with explicit rows",
        ));
    }

    let columns = if spec.columns.is_empty() {
        table
            .columns
            .iter()
            .map(|column| ColumnRef {
                table_id: table.table_id.0,
                column_id: column.column_id.0,
            })
            .collect::<Vec<_>>()
    } else {
        validate_columns(snapshot, &table, &spec.columns)?;
        spec.columns.clone()
    };

    if spec.default_values {
        return Ok(format!(
            "INSERT INTO {} DEFAULT VALUES",
            render_table_name(&table)
        ));
    }
    if spec.values.is_empty() {
        return Err(Error::new(
            ErrorCode::Misuse,
            "insert requires at least one row or DEFAULT VALUES",
        ));
    }

    let mut sql = String::new();
    write!(sql, "INSERT INTO {}", render_table_name(&table)).expect("write to string");
    sql.push_str(" (");
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&render_column_ref(snapshot, &table, *column)?);
    }
    sql.push_str(") VALUES ");

    for (row_index, row) in spec.values.iter().enumerate() {
        if row.len() != columns.len() {
            return Err(Error::new(
                ErrorCode::Mismatch,
                "insert row does not match column count",
            ));
        }
        if row_index > 0 {
            sql.push_str(", ");
        }
        sql.push('(');
        for (col_index, expr) in row.iter().enumerate() {
            if col_index > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&render_expr(snapshot, &table, expr)?);
        }
        sql.push(')');
    }

    Ok(sql)
}

fn render_update(snapshot: &SchemaSnapshot, spec: &UpdateSpec) -> Result<String> {
    let table = resolve_table(snapshot, spec.table)?;
    if spec.assignments.is_empty() {
        return Err(Error::new(
            ErrorCode::Misuse,
            "update requires at least one assignment",
        ));
    }

    let mut sql = String::new();
    write!(sql, "UPDATE {} SET ", render_table_name(&table)).expect("write to string");
    for (index, (column, expr)) in spec.assignments.iter().enumerate() {
        if index > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&render_column_ref(snapshot, &table, *column)?);
        sql.push_str(" = ");
        sql.push_str(&render_expr(snapshot, &table, expr)?);
    }
    if let Some(filter) = &spec.filter {
        sql.push_str(" WHERE ");
        sql.push_str(&render_expr(snapshot, &table, filter)?);
    }
    Ok(sql)
}

fn render_delete(snapshot: &SchemaSnapshot, spec: &DeleteSpec) -> Result<String> {
    let table = resolve_table(snapshot, spec.from)?;
    let mut sql = String::new();
    write!(sql, "DELETE FROM {}", render_table_name(&table)).expect("write to string");
    if let Some(filter) = &spec.filter {
        sql.push_str(" WHERE ");
        sql.push_str(&render_expr(snapshot, &table, filter)?);
    }
    Ok(sql)
}

fn resolve_table(snapshot: &SchemaSnapshot, table_ref: TableRef) -> Result<Arc<TableDef>> {
    if snapshot.meta.schema_epoch.0 != table_ref.schema_epoch {
        return Err(Error::new(ErrorCode::Schema, "schema epoch changed"));
    }
    snapshot
        .table_by_id(TableId(table_ref.table_id))
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "table not found"))
}

fn validate_columns(
    snapshot: &SchemaSnapshot,
    table: &TableDef,
    columns: &[ColumnRef],
) -> Result<()> {
    for column in columns {
        let _ = resolve_column(snapshot, table, *column)?;
    }
    Ok(())
}

fn resolve_column<'a>(
    _snapshot: &'a SchemaSnapshot,
    table: &'a TableDef,
    column_ref: ColumnRef,
) -> Result<&'a redlinedb_kernel::catalog::ColumnDef> {
    if column_ref.table_id != table.table_id.0 {
        return Err(Error::new(
            ErrorCode::Mismatch,
            "column belongs to a different table",
        ));
    }
    let column = table
        .columns
        .iter()
        .find(|column| column.column_id.0 == column_ref.column_id)
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "column not found"))?;
    Ok(column)
}

fn render_column_ref(
    snapshot: &SchemaSnapshot,
    table: &TableDef,
    column_ref: ColumnRef,
) -> Result<String> {
    let column = resolve_column(snapshot, table, column_ref)?;
    Ok(render_ident(&column.name))
}

fn render_table_name(table: &TableDef) -> String {
    render_ident(&table.name)
}

fn render_expr(snapshot: &SchemaSnapshot, table: &TableDef, expr: &ExprSpec) -> Result<String> {
    Ok(match expr {
        ExprSpec::Null => "NULL".to_owned(),
        ExprSpec::Boolean(value) => {
            if *value {
                "TRUE".to_owned()
            } else {
                "FALSE".to_owned()
            }
        }
        ExprSpec::Integer(value) => value.to_string(),
        ExprSpec::Real(value) => value.to_string(),
        ExprSpec::Text(value) => render_string_literal(value),
        ExprSpec::Blob(value) => render_blob_literal(value),
        ExprSpec::Param(index) => {
            if *index == 0 {
                return Err(Error::new(
                    ErrorCode::Misuse,
                    "parameter indexes are 1-based",
                ));
            }
            format!("?{index}")
        }
        ExprSpec::Column(column) => render_column_ref(snapshot, table, *column)?,
        ExprSpec::Nested(expr) => format!("({})", render_expr(snapshot, table, expr)?),
        ExprSpec::Unary { op, expr } => {
            let inner = render_expr(snapshot, table, expr)?;
            match op {
                UnaryOp::Not => format!("NOT ({inner})"),
                UnaryOp::Negate => format!("-({inner})"),
                UnaryOp::Positive => format!("+({inner})"),
                UnaryOp::BitNot => format!("~({inner})"),
            }
        }
        ExprSpec::Binary { left, op, right } => {
            let left = render_expr(snapshot, table, left)?;
            let right = render_expr(snapshot, table, right)?;
            let op = match op {
                BinaryOp::Eq => "=",
                BinaryOp::NotEq => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::LtEq => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::GtEq => ">=",
                BinaryOp::And => "AND",
                BinaryOp::Or => "OR",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Like => "LIKE",
                BinaryOp::Is => "IS",
                BinaryOp::IsNot => "IS NOT",
                BinaryOp::Concat => "||",
            };
            format!("({left} {op} {right})")
        }
        ExprSpec::Function { name, args } => {
            let mut rendered = String::new();
            rendered.push_str(&render_ident(name));
            rendered.push('(');
            for (index, arg) in args.iter().enumerate() {
                if index > 0 {
                    rendered.push_str(", ");
                }
                rendered.push_str(&render_expr(snapshot, table, arg)?);
            }
            rendered.push(')');
            rendered
        }
    })
}

fn render_ident(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        name.to_owned()
    } else {
        let escaped = name.replace('"', "\"\"");
        format!("\"{escaped}\"")
    }
}

fn render_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn render_blob_literal(value: &[u8]) -> String {
    let mut out = String::from("X'");
    for byte in value {
        write!(out, "{byte:02X}").expect("write to string");
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{Database, OpenOptions, Step, params};

    use super::*;

    fn new_db() -> (tempfile::TempDir, Database) {
        let dir = tempdir().expect("tempdir");
        let db =
            Database::open_with_options(dir.path().join("machine.redline"), OpenOptions::default())
                .expect("db");
        (dir, db)
    }

    #[test]
    fn query_spec_round_trip_select_insert_update_delete() {
        let (_dir, db) = new_db();
        let mut conn = db.connect().expect("connect");
        conn.execute(
            "CREATE TABLE widgets(id INTEGER PRIMARY KEY, name TEXT)",
            (),
        )
        .expect("create");

        let snapshot = db.inner.db.schema_snapshot();
        let table = snapshot.tables.first().expect("table");
        let id_col = table.columns[0].column_id.0;
        let name_col = table.columns[1].column_id.0;

        let insert = QuerySpec::Insert(InsertSpec {
            into: TableRef {
                table_id: table.table_id.0,
                schema_epoch: snapshot.meta.schema_epoch.0,
            },
            columns: vec![
                ColumnRef {
                    table_id: table.table_id.0,
                    column_id: id_col,
                },
                ColumnRef {
                    table_id: table.table_id.0,
                    column_id: name_col,
                },
            ],
            values: vec![
                vec![ExprSpec::Integer(1), ExprSpec::Text("Ada".to_owned())],
                vec![ExprSpec::Integer(2), ExprSpec::Text("Grace".to_owned())],
            ],
            default_values: false,
        });
        let prepared = db.prepare_query_spec(&insert).expect("prepare insert");
        assert!(prepared.sql().starts_with("INSERT INTO"));

        conn.execute(prepared.sql(), ()).expect("insert");

        let select = QuerySpec::Select(SelectSpec {
            from: TableRef {
                table_id: table.table_id.0,
                schema_epoch: snapshot.meta.schema_epoch.0,
            },
            columns: vec![ColumnRef {
                table_id: table.table_id.0,
                column_id: name_col,
            }],
            filter: Some(ExprSpec::Binary {
                left: Box::new(ExprSpec::Column(ColumnRef {
                    table_id: table.table_id.0,
                    column_id: id_col,
                })),
                op: BinaryOp::GtEq,
                right: Box::new(ExprSpec::Param(1)),
            }),
            order_by: vec![OrderSpec {
                expr: ExprSpec::Column(ColumnRef {
                    table_id: table.table_id.0,
                    column_id: id_col,
                }),
                descending: false,
                nulls_first: None,
            }],
            limit: None,
            offset: None,
            max_cost: None,
        });

        let prepared = db.prepare_query_spec(&select).expect("prepare select");
        let mut stmt = conn.prepare(prepared.sql()).expect("stmt");
        stmt.bind_all(params![1_i64]).expect("bind");
        let mut names = Vec::new();
        while let Step::Row(row) = stmt.step().expect("step") {
            names.push(row.get::<String>(0).expect("name"));
        }
        assert_eq!(names, vec!["Ada".to_owned(), "Grace".to_owned()]);

        let update = QuerySpec::Update(UpdateSpec {
            table: TableRef {
                table_id: table.table_id.0,
                schema_epoch: snapshot.meta.schema_epoch.0,
            },
            assignments: vec![(
                ColumnRef {
                    table_id: table.table_id.0,
                    column_id: name_col,
                },
                ExprSpec::Text("Ada Lovelace".to_owned()),
            )],
            filter: Some(ExprSpec::Binary {
                left: Box::new(ExprSpec::Column(ColumnRef {
                    table_id: table.table_id.0,
                    column_id: id_col,
                })),
                op: BinaryOp::Eq,
                right: Box::new(ExprSpec::Integer(1)),
            }),
        });
        let prepared = db.prepare_query_spec(&update).expect("prepare update");
        conn.execute(prepared.sql(), ()).expect("update");

        let delete = QuerySpec::Delete(DeleteSpec {
            from: TableRef {
                table_id: table.table_id.0,
                schema_epoch: snapshot.meta.schema_epoch.0,
            },
            filter: Some(ExprSpec::Binary {
                left: Box::new(ExprSpec::Column(ColumnRef {
                    table_id: table.table_id.0,
                    column_id: id_col,
                })),
                op: BinaryOp::Eq,
                right: Box::new(ExprSpec::Integer(2)),
            }),
        });
        let prepared = db.prepare_query_spec(&delete).expect("prepare delete");
        conn.execute(prepared.sql(), ()).expect("delete");
    }

    #[test]
    fn query_spec_rejects_stale_schema_epoch() {
        let (_dir, db) = new_db();
        let snapshot = db.inner.db.schema_snapshot();
        let spec = QuerySpec::Select(SelectSpec {
            from: TableRef {
                table_id: 999,
                schema_epoch: snapshot.meta.schema_epoch.0.saturating_add(1),
            },
            columns: Vec::new(),
            filter: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
            max_cost: None,
        });
        let err = db.prepare_query_spec(&spec).expect_err("stale schema");
        assert_eq!(err.code(), ErrorCode::Schema);
    }
}
