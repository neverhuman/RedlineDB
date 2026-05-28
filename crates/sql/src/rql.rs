use std::sync::Arc;

use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec, DbName,
    DropIndexSpec, DropTableSpec, ExprAst, IndexColumnSpec, IndexOrigin, OwnedValue, QualifiedName,
    SchemaSnapshot, SortDir, TableDef,
};
use serde::{Deserialize, Serialize};
use sqlparser::ast::helpers::attached_token::AttachedToken;
use sqlparser::ast::{
    BinaryOperator, CastKind, DataType, Distinct, DuplicateTreatment, Expr, Function, FunctionArg,
    FunctionArgExpr, FunctionArgumentList, FunctionArguments, GroupByExpr, Ident, Join,
    JoinConstraint, JoinOperator, LimitClause, ObjectName, ObjectNamePart, Offset, OffsetRows,
    OrderBy, OrderByExpr, OrderByKind, OrderByOptions, Query, Select, SelectFlavor, SelectItem,
    SelectItemQualifiedWildcardKind, SetExpr, TableAlias, TableFactor, TableWithJoins,
    UnaryOperator, Value, WildcardAdditionalOptions,
};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::parser::{bind_query_with_params, normalize_expr};
use crate::session::BeginMode;
use crate::statement::{
    DeletePlan, DmlValue, InsertPlan, ParamLayout, PreparedKind, PreparedTemplate, UpdatePlan,
};

mod native;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlProgram {
    #[serde(default)]
    pub statements: Vec<RqlStatement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RqlStatement {
    Begin {
        #[serde(default)]
        mode: RqlBeginMode,
    },
    Commit,
    Rollback,
    CreateTable(RqlCreateTable),
    CreateIndex(RqlCreateIndex),
    DropTable(RqlDropTable),
    DropIndex(RqlDropIndex),
    Insert(RqlInsert),
    Update(RqlUpdate),
    Delete(RqlDelete),
    Select(RqlSelect),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RqlBeginMode {
    #[default]
    Deferred,
    Immediate,
    Exclusive,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlName {
    #[serde(default)]
    pub schema: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlCreateTable {
    pub table: RqlName,
    #[serde(default)]
    pub if_not_exists: bool,
    #[serde(default)]
    pub columns: Vec<RqlColumnDef>,
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub without_rowid: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlColumnDef {
    pub name: String,
    #[serde(default)]
    pub declared_type: Option<String>,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub not_null: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub default: Option<RqlLiteral>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlCreateIndex {
    pub index: RqlName,
    pub table: RqlName,
    #[serde(default)]
    pub if_not_exists: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub columns: Vec<RqlIndexColumn>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlIndexColumn {
    pub name: String,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub collation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlDropTable {
    pub table: RqlName,
    #[serde(default)]
    pub if_exists: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlDropIndex {
    pub index: RqlName,
    #[serde(default)]
    pub if_exists: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlInsert {
    pub table: RqlName,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub values: Vec<Vec<RqlExpr>>,
    #[serde(default)]
    pub default_values: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlUpdate {
    pub table: RqlName,
    #[serde(default)]
    pub assignments: Vec<RqlUpdateAssignment>,
    #[serde(default)]
    pub filter: Option<RqlExpr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlUpdateAssignment {
    pub column: String,
    pub value: RqlExpr,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlDelete {
    pub table: RqlName,
    #[serde(default)]
    pub filter: Option<RqlExpr>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlSelect {
    #[serde(default)]
    pub distinct: bool,
    #[serde(default)]
    pub projection: Vec<RqlSelectItem>,
    #[serde(default)]
    pub from: Option<RqlTableRef>,
    #[serde(default)]
    pub joins: Vec<RqlJoin>,
    #[serde(default)]
    pub filter: Option<RqlExpr>,
    #[serde(default)]
    pub group_by: Vec<RqlExpr>,
    #[serde(default)]
    pub having: Option<RqlExpr>,
    #[serde(default)]
    pub order_by: Vec<RqlOrder>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlTableRef {
    pub name: RqlName,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RqlColumnRef {
    #[serde(default)]
    pub table: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlJoin {
    pub table: RqlTableRef,
    #[serde(default)]
    pub kind: RqlJoinKind,
    #[serde(default)]
    pub on: Option<RqlExpr>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RqlJoinKind {
    #[default]
    Inner,
    Left,
    Cross,
    Right,
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RqlSelectItem {
    Wildcard,
    QualifiedWildcard {
        table: String,
    },
    Expr {
        expr: RqlExpr,
        #[serde(default)]
        alias: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RqlOrder {
    pub expr: RqlExpr,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RqlExpr {
    Null,
    Bool {
        value: bool,
    },
    Integer {
        value: i64,
    },
    Real {
        value: f64,
    },
    Text {
        value: String,
    },
    Blob {
        bytes: Vec<u8>,
    },
    Param {
        index: usize,
    },
    Column {
        column: RqlColumnRef,
    },
    Unary {
        op: RqlUnaryOp,
        expr: Box<RqlExpr>,
    },
    Binary {
        left: Box<RqlExpr>,
        op: RqlBinaryOp,
        right: Box<RqlExpr>,
    },
    Function {
        name: String,
        #[serde(default)]
        args: Vec<RqlExpr>,
        #[serde(default)]
        distinct: bool,
    },
    CountStar,
    Cast {
        expr: Box<RqlExpr>,
        data_type: String,
    },
    IsNull {
        expr: Box<RqlExpr>,
        #[serde(default)]
        negated: bool,
    },
    Between {
        expr: Box<RqlExpr>,
        low: Box<RqlExpr>,
        high: Box<RqlExpr>,
        #[serde(default)]
        negated: bool,
    },
    InList {
        expr: Box<RqlExpr>,
        #[serde(default)]
        list: Vec<RqlExpr>,
        #[serde(default)]
        negated: bool,
    },
    InSubquery {
        expr: Box<RqlExpr>,
        select: Box<RqlSelect>,
        #[serde(default)]
        negated: bool,
    },
    Subquery {
        select: Box<RqlSelect>,
    },
    Exists {
        select: Box<RqlSelect>,
        #[serde(default)]
        negated: bool,
    },
    Nested {
        expr: Box<RqlExpr>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RqlUnaryOp {
    Not,
    Negate,
    Positive,
    BitNot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RqlBinaryOp {
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
    Concat,
    Like,
    NotLike,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RqlLiteral {
    Null,
    Bool { value: bool },
    Integer { value: i64 },
    Real { value: f64 },
    Text { value: String },
    Blob { bytes: Vec<u8> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PrepareOptions {
    pub(crate) native_select: bool,
}

impl PrepareOptions {
    pub(crate) fn from_env() -> Self {
        Self {
            native_select: native_select_enabled(),
        }
    }
}

pub(crate) fn prepare_template_with_options(
    conn: &Connection,
    statement: &RqlStatement,
    options: PrepareOptions,
) -> Result<PreparedTemplate> {
    let schema_epoch = conn.schema_epoch();
    let stats_epoch = conn.stats_epoch().0;
    let optimizer_hash = conn.optimizer_hash();
    let mut template = match statement {
        RqlStatement::Begin { mode } => bare_template(
            "begin",
            schema_epoch,
            false,
            PreparedKind::Begin(match mode {
                RqlBeginMode::Deferred => BeginMode::Deferred,
                RqlBeginMode::Immediate => BeginMode::Immediate,
                RqlBeginMode::Exclusive => BeginMode::Exclusive,
            }),
        ),
        RqlStatement::Commit => bare_template("commit", schema_epoch, false, PreparedKind::Commit),
        RqlStatement::Rollback => {
            bare_template("rollback", schema_epoch, false, PreparedKind::Rollback)
        }
        RqlStatement::CreateTable(create) => lower_create_table(schema_epoch, create)?,
        RqlStatement::CreateIndex(create) => lower_create_index(schema_epoch, create)?,
        RqlStatement::DropTable(drop) => bare_template(
            "drop_table",
            schema_epoch,
            false,
            PreparedKind::DropTable(DropTableSpec {
                name: qualified_name(&drop.table),
                if_exists: drop.if_exists,
            }),
        ),
        RqlStatement::DropIndex(drop) => bare_template(
            "drop_index",
            schema_epoch,
            false,
            PreparedKind::DropIndex(DropIndexSpec {
                name: qualified_name(&drop.index),
                if_exists: drop.if_exists,
            }),
        ),
        RqlStatement::Insert(insert) => lower_insert(conn.schema_snapshot(), schema_epoch, insert)?,
        RqlStatement::Update(update) => lower_update(conn.schema_snapshot(), schema_epoch, update)?,
        RqlStatement::Delete(delete) => lower_delete(conn.schema_snapshot(), schema_epoch, delete)?,
        RqlStatement::Select(select) => {
            if options.native_select
                && let Some(template) =
                    native::lower_native_select(conn.schema_snapshot(), schema_epoch, select)?
            {
                template
            } else {
                let mut params = ParamLayout::default();
                bind_query_with_params(
                    conn,
                    conn.schema_snapshot(),
                    schema_epoch,
                    rql_sql("select").as_ref(),
                    select_query(select)?,
                    &mut params,
                )?
            }
        }
    };
    template.stats_epoch = stats_epoch;
    template.optimizer_hash = optimizer_hash;
    Ok(template)
}

pub(crate) fn template_cache_enabled() -> bool {
    std::env::var_os("REDLINE_RQL_TEMPLATE_CACHE")
        .map(|value| value != "0" && !value.is_empty())
        .unwrap_or(false)
}

pub(crate) fn native_select_enabled() -> bool {
    std::env::var_os("REDLINE_RQL_NATIVE_SELECT")
        .map(|value| value != "0" && !value.is_empty())
        .unwrap_or(false)
}

pub(crate) fn cache_key(statement: &RqlStatement, options: PrepareOptions) -> Result<Arc<str>> {
    let json = serde_json::to_string(statement)
        .map_err(|err| Error::Bind(format!("failed to build RQL cache key: {err}")))?;
    let route = if matches!(statement, RqlStatement::Select(_)) {
        if options.native_select {
            "cache:native_select=1"
        } else {
            "cache:native_select=0"
        }
    } else {
        "cache"
    };
    Ok(Arc::from(format!(
        "{}{route}:{json}",
        crate::statement::RQL_MARKER_SQL_PREFIX
    )))
}

fn lower_create_table(
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    create: &RqlCreateTable,
) -> Result<PreparedTemplate> {
    let columns = create
        .columns
        .iter()
        .map(|column| {
            let mut constraints = Vec::new();
            if column.primary_key {
                constraints.push(ColumnConstraintSpec::PrimaryKey {
                    sort_dir: SortDir::Asc,
                    conflict: ConflictAction::Abort,
                });
            }
            if column.not_null {
                constraints.push(ColumnConstraintSpec::NotNull {
                    conflict: ConflictAction::Abort,
                });
            }
            if column.unique {
                constraints.push(ColumnConstraintSpec::Unique {
                    conflict: ConflictAction::Abort,
                });
            }
            let default_value = column.default.as_ref().map(literal_owned_value);
            if let Some(value) = &default_value {
                constraints.push(ColumnConstraintSpec::Default {
                    expr: ExprAst::Const(value.clone()),
                    normalized_sql: literal_expr(&literal_from_owned(value.clone())).to_string(),
                });
            }
            Ok(ColumnSpec {
                name: DbName::new(&column.name),
                declared_type: column.declared_type.clone(),
                constraints,
                collation: None,
                default_value,
                autoincrement: false,
                generated: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(bare_template(
        "create_table",
        schema_epoch,
        false,
        PreparedKind::CreateTable(CreateTableSpec {
            schema: create.table.schema.as_ref().map(DbName::new),
            name: DbName::new(&create.table.name),
            if_not_exists: create.if_not_exists,
            columns,
            constraints: Vec::new(),
            strict: create.strict,
            without_rowid: create.without_rowid,
            normalized_sql: None,
        }),
    ))
}

fn lower_create_index(
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    create: &RqlCreateIndex,
) -> Result<PreparedTemplate> {
    if create.columns.is_empty() {
        return Err(Error::Bind(
            "RQL CREATE INDEX requires at least one column".to_owned(),
        ));
    }
    let columns = create
        .columns
        .iter()
        .map(|column| IndexColumnSpec {
            name: DbName::new(&column.name),
            sort_dir: if column.descending {
                SortDir::Desc
            } else {
                SortDir::Asc
            },
            collation: column.collation.clone(),
            expr_sql: None,
            expr_referenced_cols: Vec::new(),
        })
        .collect();
    Ok(bare_template(
        "create_index",
        schema_epoch,
        false,
        PreparedKind::CreateIndex(CreateIndexSpec {
            schema: create.index.schema.as_ref().map(DbName::new),
            name: DbName::new(&create.index.name),
            if_not_exists: create.if_not_exists,
            table: qualified_name(&create.table),
            unique: create.unique,
            columns,
            origin: IndexOrigin::User,
            normalized_sql: None,
            predicate_sql: None,
        }),
    ))
}

fn lower_insert(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    insert: &RqlInsert,
) -> Result<PreparedTemplate> {
    let table = resolve_table(&schema, &insert.table)?;
    if insert.default_values && !insert.values.is_empty() {
        return Err(Error::Bind(
            "RQL INSERT cannot combine default_values with rows".to_owned(),
        ));
    }
    if !insert.default_values && insert.values.is_empty() {
        return Err(Error::Bind(
            "RQL INSERT requires rows or default_values".to_owned(),
        ));
    }
    let columns = if insert.columns.is_empty() {
        (0..table.columns.len()).collect::<Vec<_>>()
    } else {
        insert
            .columns
            .iter()
            .map(|name| column_ordinal(&table, name))
            .collect::<Result<Vec<_>>>()?
    };
    let mut params = ParamLayout::default();
    let mut rows = Vec::with_capacity(insert.values.len());
    for row in &insert.values {
        if row.len() != columns.len() {
            return Err(Error::Bind(
                "RQL INSERT row length does not match column count".to_owned(),
            ));
        }
        rows.push(
            row.iter()
                .map(|expr| Ok(DmlValue::Expr(normalized_expr(expr, &mut params)?)))
                .collect::<Result<Vec<_>>>()?,
        );
    }
    Ok(template(
        "insert",
        schema_epoch,
        false,
        params,
        Arc::from([]),
        PreparedKind::Insert(InsertPlan {
            table,
            columns,
            rows,
            source_select: None,
            default_values: insert.default_values,
            returning: None,
            conflict: None,
        }),
    ))
}

fn lower_update(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    update: &RqlUpdate,
) -> Result<PreparedTemplate> {
    let table = resolve_table(&schema, &update.table)?;
    if update.assignments.is_empty() {
        return Err(Error::Bind("RQL UPDATE requires assignments".to_owned()));
    }
    let mut params = ParamLayout::default();
    let assignments = update
        .assignments
        .iter()
        .map(|assignment| {
            Ok((
                column_ordinal(&table, &assignment.column)?,
                DmlValue::Expr(normalized_expr(&assignment.value, &mut params)?),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let selection = update
        .filter
        .as_ref()
        .map(|expr| normalized_expr(expr, &mut params))
        .transpose()?;
    Ok(template(
        "update",
        schema_epoch,
        false,
        params,
        Arc::from([]),
        PreparedKind::Update(UpdatePlan {
            table,
            assignments,
            selection,
            returning: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }),
    ))
}

fn lower_delete(
    schema: Arc<SchemaSnapshot>,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    delete: &RqlDelete,
) -> Result<PreparedTemplate> {
    let table = resolve_table(&schema, &delete.table)?;
    let mut params = ParamLayout::default();
    let selection = delete
        .filter
        .as_ref()
        .map(|expr| normalized_expr(expr, &mut params))
        .transpose()?;
    Ok(template(
        "delete",
        schema_epoch,
        false,
        params,
        Arc::from([]),
        PreparedKind::Delete(DeletePlan {
            table,
            selection,
            returning: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }),
    ))
}

fn select_query(select: &RqlSelect) -> Result<Query> {
    let projection = if select.projection.is_empty() {
        vec![SelectItem::Wildcard(WildcardAdditionalOptions::default())]
    } else {
        select
            .projection
            .iter()
            .map(select_item)
            .collect::<Result<Vec<_>>>()?
    };
    let from = match &select.from {
        Some(table) => vec![TableWithJoins {
            relation: table_factor(table),
            joins: select.joins.iter().map(join).collect::<Result<Vec<_>>>()?,
        }],
        None => {
            if !select.joins.is_empty() {
                return Err(Error::Bind("RQL JOIN requires a base table".to_owned()));
            }
            Vec::new()
        }
    };
    let order_by = if select.order_by.is_empty() {
        None
    } else {
        Some(OrderBy {
            kind: OrderByKind::Expressions(
                select
                    .order_by
                    .iter()
                    .map(|order| {
                        Ok(OrderByExpr {
                            expr: expr(&order.expr)?,
                            options: OrderByOptions {
                                asc: Some(!order.descending),
                                nulls_first: order.nulls_first,
                            },
                            with_fill: None,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
            interpolate: None,
        })
    };
    let limit_clause = match (select.limit, select.offset) {
        (None, None) => None,
        (limit, offset) => Some(LimitClause::LimitOffset {
            limit: limit.map(u64_expr),
            offset: offset.map(|value| Offset {
                value: u64_expr(value),
                rows: OffsetRows::None,
            }),
            limit_by: Vec::new(),
        }),
    };
    Ok(Query {
        with: None,
        body: Box::new(SetExpr::Select(Box::new(Select {
            select_token: AttachedToken::empty(),
            optimizer_hint: None,
            distinct: select.distinct.then_some(Distinct::Distinct),
            select_modifiers: None,
            top: None,
            top_before_distinct: false,
            projection,
            exclude: None,
            into: None,
            from,
            lateral_views: Vec::new(),
            prewhere: None,
            selection: select.filter.as_ref().map(expr).transpose()?,
            connect_by: Vec::new(),
            group_by: GroupByExpr::Expressions(
                select
                    .group_by
                    .iter()
                    .map(expr)
                    .collect::<Result<Vec<_>>>()?,
                Vec::new(),
            ),
            cluster_by: Vec::new(),
            distribute_by: Vec::new(),
            sort_by: Vec::new(),
            having: select.having.as_ref().map(expr).transpose()?,
            named_window: Vec::new(),
            qualify: None,
            window_before_qualify: false,
            value_table_mode: None,
            flavor: SelectFlavor::Standard,
        }))),
        order_by,
        limit_clause,
        fetch: None,
        locks: Vec::new(),
        for_clause: None,
        settings: None,
        format_clause: None,
        pipe_operators: Vec::new(),
    })
}

fn select_item(item: &RqlSelectItem) -> Result<SelectItem> {
    Ok(match item {
        RqlSelectItem::Wildcard => SelectItem::Wildcard(WildcardAdditionalOptions::default()),
        RqlSelectItem::QualifiedWildcard { table } => SelectItem::QualifiedWildcard(
            SelectItemQualifiedWildcardKind::ObjectName(sql_name(&RqlName {
                schema: None,
                name: table.clone(),
            })),
            WildcardAdditionalOptions::default(),
        ),
        RqlSelectItem::Expr { expr: item, alias } => match alias {
            Some(alias) => SelectItem::ExprWithAlias {
                expr: expr(item)?,
                alias: Ident::new(alias),
            },
            None => SelectItem::UnnamedExpr(expr(item)?),
        },
    })
}

fn join(join: &RqlJoin) -> Result<Join> {
    let constraint = match &join.on {
        Some(on_expr) => JoinConstraint::On(expr(on_expr)?),
        None => JoinConstraint::None,
    };
    let join_operator = match join.kind {
        RqlJoinKind::Inner => JoinOperator::Inner(constraint),
        RqlJoinKind::Left => JoinOperator::LeftOuter(constraint),
        RqlJoinKind::Cross => JoinOperator::CrossJoin(constraint),
        RqlJoinKind::Right => JoinOperator::RightOuter(constraint),
        RqlJoinKind::Full => JoinOperator::FullOuter(constraint),
    };
    Ok(Join {
        relation: table_factor(&join.table),
        global: false,
        join_operator,
    })
}

fn table_factor(table: &RqlTableRef) -> TableFactor {
    TableFactor::Table {
        name: sql_name(&table.name),
        alias: table.alias.as_ref().map(|alias| TableAlias {
            explicit: false,
            name: Ident::new(alias),
            columns: Vec::new(),
        }),
        args: None,
        with_hints: Vec::new(),
        version: None,
        with_ordinality: false,
        partitions: Vec::new(),
        json_path: None,
        sample: None,
        index_hints: Vec::new(),
    }
}

fn normalized_expr(value: &RqlExpr, params: &mut ParamLayout) -> Result<Expr> {
    normalize_expr(expr(value)?, params)
}

fn expr(value: &RqlExpr) -> Result<Expr> {
    Ok(match value {
        RqlExpr::Null => Expr::value(Value::Null),
        RqlExpr::Bool { value } => Expr::value(Value::Boolean(*value)),
        RqlExpr::Integer { value } => Expr::value(Value::Number(value.to_string(), false)),
        RqlExpr::Real { value } => Expr::value(Value::Number(real_number(*value), false)),
        RqlExpr::Text { value } => Expr::value(Value::SingleQuotedString(value.clone())),
        RqlExpr::Blob { bytes } => Expr::value(Value::HexStringLiteral(hex_upper(bytes))),
        RqlExpr::Param { index } => {
            if *index == 0 {
                return Err(Error::ParameterOutOfRange(0));
            }
            Expr::value(crate::parser::bind::into_bind_value(format!("?{index}")))
        }
        RqlExpr::Column { column } => column_expr(column),
        RqlExpr::Unary { op, expr: inner } => Expr::UnaryOp {
            op: match op {
                RqlUnaryOp::Not => UnaryOperator::Not,
                RqlUnaryOp::Negate => UnaryOperator::Minus,
                RqlUnaryOp::Positive => UnaryOperator::Plus,
                RqlUnaryOp::BitNot => UnaryOperator::BitwiseNot,
            },
            expr: Box::new(expr(inner)?),
        },
        RqlExpr::Binary { left, op, right } => match op {
            RqlBinaryOp::Like | RqlBinaryOp::NotLike => Expr::Like {
                negated: matches!(op, RqlBinaryOp::NotLike),
                any: false,
                expr: Box::new(expr(left)?),
                pattern: Box::new(expr(right)?),
                escape_char: None,
            },
            _ => Expr::BinaryOp {
                left: Box::new(expr(left)?),
                op: binary_op(*op),
                right: Box::new(expr(right)?),
            },
        },
        RqlExpr::Function {
            name,
            args,
            distinct,
        } => Expr::Function(function_expr(
            name,
            args.iter()
                .map(|arg| Ok(FunctionArg::Unnamed(FunctionArgExpr::Expr(expr(arg)?))))
                .collect::<Result<Vec<_>>>()?,
            *distinct,
        )),
        RqlExpr::CountStar => Expr::Function(function_expr(
            "count",
            vec![FunctionArg::Unnamed(FunctionArgExpr::Wildcard)],
            false,
        )),
        RqlExpr::Cast {
            expr: inner,
            data_type,
        } => Expr::Cast {
            expr: Box::new(expr(inner)?),
            data_type: DataType::Custom(sql_name_part(data_type), Vec::new()),
            kind: CastKind::Cast,
            format: None,
            array: false,
        },
        RqlExpr::IsNull {
            expr: inner,
            negated,
        } => {
            if *negated {
                Expr::IsNotNull(Box::new(expr(inner)?))
            } else {
                Expr::IsNull(Box::new(expr(inner)?))
            }
        }
        RqlExpr::Between {
            expr: inner,
            low,
            high,
            negated,
        } => Expr::Between {
            expr: Box::new(expr(inner)?),
            negated: *negated,
            low: Box::new(expr(low)?),
            high: Box::new(expr(high)?),
        },
        RqlExpr::InList {
            expr: inner,
            list,
            negated,
        } => Expr::InList {
            expr: Box::new(expr(inner)?),
            list: list.iter().map(expr).collect::<Result<Vec<_>>>()?,
            negated: *negated,
        },
        RqlExpr::InSubquery {
            expr: inner,
            select,
            negated,
        } => Expr::InSubquery {
            expr: Box::new(expr(inner)?),
            subquery: Box::new(select_query(select)?),
            negated: *negated,
        },
        RqlExpr::Subquery { select } => Expr::Subquery(Box::new(select_query(select)?)),
        RqlExpr::Exists { select, negated } => Expr::Exists {
            subquery: Box::new(select_query(select)?),
            negated: *negated,
        },
        RqlExpr::Nested { expr: inner } => Expr::Nested(Box::new(expr(inner)?)),
    })
}

fn binary_op(op: RqlBinaryOp) -> BinaryOperator {
    match op {
        RqlBinaryOp::Eq => BinaryOperator::Eq,
        RqlBinaryOp::NotEq => BinaryOperator::NotEq,
        RqlBinaryOp::Lt => BinaryOperator::Lt,
        RqlBinaryOp::LtEq => BinaryOperator::LtEq,
        RqlBinaryOp::Gt => BinaryOperator::Gt,
        RqlBinaryOp::GtEq => BinaryOperator::GtEq,
        RqlBinaryOp::And => BinaryOperator::And,
        RqlBinaryOp::Or => BinaryOperator::Or,
        RqlBinaryOp::Add => BinaryOperator::Plus,
        RqlBinaryOp::Sub => BinaryOperator::Minus,
        RqlBinaryOp::Mul => BinaryOperator::Multiply,
        RqlBinaryOp::Div => BinaryOperator::Divide,
        RqlBinaryOp::Mod => BinaryOperator::Modulo,
        RqlBinaryOp::Concat => BinaryOperator::StringConcat,
        RqlBinaryOp::Like | RqlBinaryOp::NotLike => {
            unreachable!("LIKE lowers through Expr::Like")
        }
    }
}

fn function_expr(name: impl AsRef<str>, args: Vec<FunctionArg>, distinct: bool) -> Function {
    Function {
        name: sql_name_part(name.as_ref()),
        uses_odbc_syntax: false,
        parameters: FunctionArguments::None,
        args: FunctionArguments::List(FunctionArgumentList {
            duplicate_treatment: distinct.then_some(DuplicateTreatment::Distinct),
            args,
            clauses: Vec::new(),
        }),
        filter: None,
        null_treatment: None,
        over: None,
        within_group: Vec::new(),
    }
}

fn column_expr(column: &RqlColumnRef) -> Expr {
    match &column.table {
        Some(table) => Expr::CompoundIdentifier(vec![Ident::new(table), Ident::new(&column.name)]),
        None => Expr::Identifier(Ident::new(&column.name)),
    }
}

fn u64_expr(value: u64) -> Expr {
    Expr::value(Value::Number(value.to_string(), false))
}

fn resolve_table(schema: &SchemaSnapshot, name: &RqlName) -> Result<Arc<TableDef>> {
    if let Some(schema_name) = &name.schema
        && !schema_name.eq_ignore_ascii_case("main")
    {
        return Err(Error::UnsupportedSql(format!(
            "RQL schema `{schema_name}` is not supported"
        )));
    }
    let schema_id = schema
        .lookup_namespace("main")
        .ok_or_else(|| Error::UnknownTable(name.name.clone()))?;
    schema
        .lookup_table(schema_id, &name.name)
        .ok_or_else(|| Error::UnknownTable(name.name.clone()))
}

fn column_ordinal(table: &TableDef, name: &str) -> Result<usize> {
    table
        .columns
        .iter()
        .position(|column| column.folded.as_ref().eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::UnknownColumn(name.to_owned()))
}

fn qualified_name(name: &RqlName) -> QualifiedName {
    QualifiedName {
        schema: DbName::new(name.schema.as_deref().unwrap_or("main")),
        name: DbName::new(&name.name),
    }
}

fn sql_name(name: &RqlName) -> ObjectName {
    match &name.schema {
        Some(schema) => ObjectName::from(vec![Ident::new(schema), Ident::new(&name.name)]),
        None => sql_name_part(&name.name),
    }
}

fn sql_name_part(name: impl AsRef<str>) -> ObjectName {
    ObjectName(vec![ObjectNamePart::Identifier(Ident::new(name.as_ref()))])
}

fn literal_owned_value(literal: &RqlLiteral) -> OwnedValue {
    match literal {
        RqlLiteral::Null => OwnedValue::Null,
        RqlLiteral::Bool { value } => OwnedValue::Integer(i64::from(*value)),
        RqlLiteral::Integer { value } => OwnedValue::Integer(*value),
        RqlLiteral::Real { value } => OwnedValue::Real(*value),
        RqlLiteral::Text { value } => OwnedValue::Text(Arc::from(value.as_str())),
        RqlLiteral::Blob { bytes } => OwnedValue::Blob(Arc::from(bytes.as_slice())),
    }
}

fn literal_from_owned(value: OwnedValue) -> RqlLiteral {
    match value {
        OwnedValue::Null => RqlLiteral::Null,
        OwnedValue::Integer(value) => RqlLiteral::Integer { value },
        OwnedValue::Real(value) => RqlLiteral::Real { value },
        OwnedValue::Text(value) => RqlLiteral::Text {
            value: value.to_string(),
        },
        OwnedValue::Blob(value) => RqlLiteral::Blob {
            bytes: value.to_vec(),
        },
    }
}

fn literal_expr(literal: &RqlLiteral) -> Expr {
    match literal {
        RqlLiteral::Null => Expr::value(Value::Null),
        RqlLiteral::Bool { value } => Expr::value(Value::Boolean(*value)),
        RqlLiteral::Integer { value } => Expr::value(Value::Number(value.to_string(), false)),
        RqlLiteral::Real { value } => Expr::value(Value::Number(real_number(*value), false)),
        RqlLiteral::Text { value } => Expr::value(Value::SingleQuotedString(value.clone())),
        RqlLiteral::Blob { bytes } => Expr::value(Value::HexStringLiteral(hex_upper(bytes))),
    }
}

fn bare_template(
    label: &'static str,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    readonly: bool,
    kind: PreparedKind,
) -> PreparedTemplate {
    template(
        label,
        schema_epoch,
        readonly,
        ParamLayout::default(),
        Arc::from([]),
        kind,
    )
}

fn template(
    label: &'static str,
    schema_epoch: redlinedb_kernel::catalog::SchemaEpoch,
    readonly: bool,
    param_layout: ParamLayout,
    output_columns: Arc<[String]>,
    kind: PreparedKind,
) -> PreparedTemplate {
    PreparedTemplate {
        sql: rql_sql(label),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout,
        output_columns,
        readonly,
        kind,
    }
}

fn rql_sql(label: &'static str) -> Arc<str> {
    Arc::from(format!(
        "{}{}",
        crate::statement::RQL_MARKER_SQL_PREFIX,
        label
    ))
}

fn hex_upper(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02X}");
    }
    out
}

fn real_number(value: f64) -> String {
    let raw = value.to_string();
    if raw.contains('.') || raw.contains('e') || raw.contains('E') {
        raw
    } else {
        format!("{raw}.0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{Database, DbOptions};
    use crate::statement::Step;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        old: Vec<(&'static str, Option<std::ffi::OsString>)>,
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set(name: &'static str, value: Option<&str>) -> Self {
            Self::set_many(&[(name, value)])
        }

        fn set_many(vars: &[(&'static str, Option<&str>)]) -> Self {
            let guard = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let old = vars
                .iter()
                .map(|(name, _)| (*name, std::env::var_os(name)))
                .collect::<Vec<_>>();
            // SAFETY: this test module serializes all mutations to these env vars.
            unsafe {
                for (name, value) in vars {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
            Self { old, _guard: guard }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: this test module serializes all mutations to these env vars.
            unsafe {
                for (name, value) in &self.old {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    fn memory_conn() -> Arc<Connection> {
        Database::create_in_memory(DbOptions::default())
            .expect("db")
            .connect()
    }

    fn create_items(conn: &Arc<Connection>) {
        conn.execute("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT, score INTEGER)")
            .expect("create items");
        conn.execute("INSERT INTO items(id, name, score) VALUES (1, 'Bob', 10), (2, 'Ada', 20), (3, 'Zoe', 30)")
            .expect("seed items");
    }

    fn items_table_ref(alias: Option<&str>) -> RqlTableRef {
        RqlTableRef {
            name: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            alias: alias.map(str::to_owned),
        }
    }

    fn native_select_statement() -> RqlStatement {
        RqlStatement::Select(RqlSelect {
            distinct: false,
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::Column {
                    column: RqlColumnRef {
                        table: Some("i".to_owned()),
                        name: "name".to_owned(),
                    },
                },
                alias: None,
            }],
            from: Some(items_table_ref(Some("i"))),
            joins: Vec::new(),
            filter: Some(RqlExpr::Binary {
                left: Box::new(RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "score".to_owned(),
                    },
                }),
                op: RqlBinaryOp::Gt,
                right: Box::new(RqlExpr::Param { index: 1 }),
            }),
            group_by: Vec::new(),
            having: None,
            order_by: vec![RqlOrder {
                expr: RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "name".to_owned(),
                    },
                },
                descending: true,
                nulls_first: None,
            }],
            limit: Some(1),
            offset: Some(0),
        })
    }

    fn collect_first_text(
        conn: &Arc<Connection>,
        statement: &RqlStatement,
        min_score: i64,
    ) -> String {
        let mut stmt = conn.prepare_rql(statement).expect("prepare rql");
        stmt.bind_i64(1, min_score).expect("bind min score");
        assert!(matches!(stmt.step().expect("row"), Step::Row));
        stmt.column_text(0).expect("text").to_owned()
    }

    #[test]
    fn rql_create_insert_select_lowers_without_sql_parse() {
        let conn = memory_conn();
        let create = RqlStatement::CreateTable(RqlCreateTable {
            table: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            if_not_exists: false,
            columns: vec![
                RqlColumnDef {
                    name: "id".to_owned(),
                    declared_type: Some("INTEGER".to_owned()),
                    primary_key: true,
                    not_null: false,
                    unique: false,
                    default: None,
                },
                RqlColumnDef {
                    name: "name".to_owned(),
                    declared_type: Some("TEXT".to_owned()),
                    primary_key: false,
                    not_null: true,
                    unique: false,
                    default: None,
                },
            ],
            strict: false,
            without_rowid: false,
        });
        let mut stmt = conn.prepare_rql(&create).expect("create");
        assert!(matches!(stmt.step().expect("step"), Step::Done));

        let insert = RqlStatement::Insert(RqlInsert {
            table: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            columns: vec!["id".to_owned(), "name".to_owned()],
            values: vec![vec![
                RqlExpr::Integer { value: 1 },
                RqlExpr::Text {
                    value: "Ada".to_owned(),
                },
            ]],
            default_values: false,
        });
        let mut stmt = conn.prepare_rql(&insert).expect("insert");
        assert!(matches!(stmt.step().expect("step"), Step::Done));

        let select = RqlStatement::Select(RqlSelect {
            distinct: false,
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "name".to_owned(),
                    },
                },
                alias: None,
            }],
            from: Some(items_table_ref(None)),
            joins: Vec::new(),
            filter: Some(RqlExpr::Binary {
                left: Box::new(RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "id".to_owned(),
                    },
                }),
                op: RqlBinaryOp::Eq,
                right: Box::new(RqlExpr::Integer { value: 1 }),
            }),
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        });
        let mut stmt = conn.prepare_rql(&select).expect("select");
        assert_eq!(stmt.column_count(), 1);
        assert!(matches!(stmt.step().expect("row"), Step::Row));
        assert_eq!(stmt.column_text(0).expect("text"), "Ada");
    }

    #[test]
    fn rql_native_select_matches_sql_ast_path_for_filter_order_limit() {
        let conn = memory_conn();
        create_items(&conn);
        let select = native_select_statement();

        let _sql_route = EnvGuard::set("REDLINE_RQL_NATIVE_SELECT", None);
        let expected = collect_first_text(&conn, &select, 10);
        drop(_sql_route);

        let _native_route = EnvGuard::set("REDLINE_RQL_NATIVE_SELECT", Some("1"));
        let actual = collect_first_text(&conn, &select, 10);
        let template = conn
            .prepare_rql(&select)
            .expect("native template")
            .template();

        assert_eq!(actual, expected);
        assert!(template.sql.as_ref().ends_with("select_native"));
    }

    #[test]
    fn rql_native_select_preserves_params_and_output_names() {
        let _env = EnvGuard::set("REDLINE_RQL_NATIVE_SELECT", Some("1"));
        let conn = memory_conn();
        create_items(&conn);
        let select = RqlStatement::Select(RqlSelect {
            distinct: false,
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "name".to_owned(),
                    },
                },
                alias: Some("item_name".to_owned()),
            }],
            from: Some(items_table_ref(None)),
            joins: Vec::new(),
            filter: Some(RqlExpr::Binary {
                left: Box::new(RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "id".to_owned(),
                    },
                }),
                op: RqlBinaryOp::Eq,
                right: Box::new(RqlExpr::Param { index: 3 }),
            }),
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        });

        let mut stmt = conn.prepare_rql(&select).expect("native select");
        assert_eq!(stmt.parameter_count(), 3);
        assert_eq!(stmt.parameter_index("?3"), Some(3));
        assert_eq!(stmt.column_count(), 1);
        assert_eq!(stmt.column_name(0), "item_name");
        assert!(stmt.template().sql.as_ref().ends_with("select_native"));
        stmt.bind_i64(3, 2).expect("bind id");
        assert!(matches!(stmt.step().expect("row"), Step::Row));
        assert_eq!(stmt.column_text(0).expect("name"), "Ada");
    }

    #[test]
    fn rql_native_select_falls_back_for_join_subquery() {
        let _env = EnvGuard::set("REDLINE_RQL_NATIVE_SELECT", Some("1"));
        let conn = memory_conn();
        create_items(&conn);
        let aggregate_select = RqlSelect {
            distinct: false,
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::CountStar,
                alias: None,
            }],
            from: Some(items_table_ref(None)),
            joins: Vec::new(),
            filter: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        };
        // Inner joins are now supported by the native route; Right/Full joins
        // are still unsupported and trigger fallback to the SQL route.
        let right_join_select = RqlSelect {
            joins: vec![RqlJoin {
                table: RqlTableRef {
                    name: RqlName {
                        schema: None,
                        name: "items".to_owned(),
                    },
                    alias: Some("i2".to_owned()),
                },
                kind: RqlJoinKind::Right,
                on: None,
            }],
            ..aggregate_select.clone()
        };
        assert!(!native::native_select_shape_supported(
            conn.schema_snapshot().as_ref(),
            &right_join_select
        ));

        let subquery_select = RqlSelect {
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::Subquery {
                    select: Box::new(RqlSelect {
                        distinct: false,
                        projection: vec![RqlSelectItem::Expr {
                            expr: RqlExpr::Integer { value: 1 },
                            alias: None,
                        }],
                        from: None,
                        joins: Vec::new(),
                        filter: None,
                        group_by: Vec::new(),
                        having: None,
                        order_by: Vec::new(),
                        limit: None,
                        offset: None,
                    }),
                },
                alias: None,
            }],
            from: Some(items_table_ref(None)),
            joins: Vec::new(),
            ..aggregate_select.clone()
        };
        assert!(!native::native_select_shape_supported(
            conn.schema_snapshot().as_ref(),
            &subquery_select
        ));
    }

    #[test]
    fn rql_native_select_template_cache_is_gate_separated() {
        let conn = memory_conn();
        create_items(&conn);
        let select = native_select_statement();

        let _sql_route = EnvGuard::set_many(&[
            ("REDLINE_RQL_TEMPLATE_CACHE", Some("1")),
            ("REDLINE_RQL_NATIVE_SELECT", None),
        ]);
        let sql_template = conn.prepare_rql(&select).expect("sql route").template();
        assert!(sql_template.sql.as_ref().ends_with("select"));
        assert!(!sql_template.sql.as_ref().ends_with("select_native"));
        drop(_sql_route);

        let _native_route = EnvGuard::set_many(&[
            ("REDLINE_RQL_TEMPLATE_CACHE", Some("1")),
            ("REDLINE_RQL_NATIVE_SELECT", Some("1")),
        ]);
        let native_template = conn.prepare_rql(&select).expect("native route").template();
        assert!(native_template.sql.as_ref().ends_with("select_native"));
        assert!(!Arc::ptr_eq(&sql_template, &native_template));
    }

    #[test]
    fn rql_template_cache_reuses_only_when_enabled() {
        let conn = memory_conn();
        let create = RqlStatement::CreateTable(RqlCreateTable {
            table: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            if_not_exists: false,
            columns: vec![RqlColumnDef {
                name: "id".to_owned(),
                declared_type: Some("INTEGER".to_owned()),
                primary_key: true,
                not_null: false,
                unique: false,
                default: None,
            }],
            strict: false,
            without_rowid: false,
        });
        let mut stmt = conn.prepare_rql(&create).expect("create");
        assert!(matches!(stmt.step().expect("step"), Step::Done));

        let select = RqlStatement::Select(RqlSelect {
            distinct: false,
            projection: vec![RqlSelectItem::Expr {
                expr: RqlExpr::Column {
                    column: RqlColumnRef {
                        table: None,
                        name: "id".to_owned(),
                    },
                },
                alias: None,
            }],
            from: Some(RqlTableRef {
                name: RqlName {
                    schema: None,
                    name: "items".to_owned(),
                },
                alias: None,
            }),
            joins: Vec::new(),
            filter: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        });

        {
            let _env = EnvGuard::set("REDLINE_RQL_TEMPLATE_CACHE", None);
            let first = conn.prepare_rql(&select).expect("first").template();
            let second = conn.prepare_rql(&select).expect("second").template();
            assert!(!Arc::ptr_eq(&first, &second));
        }
        {
            let _env = EnvGuard::set("REDLINE_RQL_TEMPLATE_CACHE", Some("1"));
            let first = conn.prepare_rql(&select).expect("cached first").template();
            let second = conn.prepare_rql(&select).expect("cached second").template();
            assert!(Arc::ptr_eq(&first, &second));
        }
    }

    #[test]
    fn rql_template_cache_preserves_savepoint_mutation_rejection() {
        let _env = EnvGuard::set("REDLINE_RQL_TEMPLATE_CACHE", Some("1"));
        let conn = memory_conn();
        let create = RqlStatement::CreateTable(RqlCreateTable {
            table: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            if_not_exists: false,
            columns: vec![RqlColumnDef {
                name: "id".to_owned(),
                declared_type: Some("INTEGER".to_owned()),
                primary_key: true,
                not_null: false,
                unique: false,
                default: None,
            }],
            strict: false,
            without_rowid: false,
        });
        let mut stmt = conn.prepare_rql(&create).expect("create");
        assert!(matches!(stmt.step().expect("step"), Step::Done));

        let insert = RqlStatement::Insert(RqlInsert {
            table: RqlName {
                schema: None,
                name: "items".to_owned(),
            },
            columns: vec!["id".to_owned()],
            values: vec![vec![RqlExpr::Integer { value: 1 }]],
            default_values: false,
        });
        let cached = conn.prepare_rql(&insert).expect("cache insert").template();
        assert!(!cached.readonly);

        let mut savepoint = conn.prepare("SAVEPOINT rql_cache").expect("savepoint");
        assert!(matches!(
            savepoint.step().expect("savepoint step"),
            Step::Done
        ));

        let err = match conn.prepare_rql(&insert) {
            Ok(_) => panic!("cached RQL mutation inside SAVEPOINT must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("SAVEPOINT"),
            "unexpected error: {err:?}"
        );
    }
}
