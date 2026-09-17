use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde_json::{Value as JsonValue, json};
use sqlparser::ast::{
    AssignmentTarget, BinaryOperator, ColumnOption, DataType, Distinct, DuplicateTreatment, Expr,
    FunctionArg, FunctionArgExpr, FunctionArguments, Ident, JoinConstraint, JoinOperator,
    LimitClause, ObjectName, ObjectNamePart, ObjectType, OrderByKind, Query, SelectItem, SetExpr,
    Statement, TableAlias, TableFactor, TableWithJoins, UnaryOperator, Value,
};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use super::case::Case;
use super::engine::{self, EngineOutput, EngineSpec};
use super::normalize::normalize_output;
use super::report;
use super::runner::{self, RunSummary};
use super::text::sanitize_identifier;

const RQL_PHASE1_CATEGORIES: &[&str] = &[
    "GEN_SQL_AGGREGATE",
    "GEN_SQL_DML",
    "GEN_SQL_JOIN_SUBQUERY",
    "GEN_SQL_JSON",
    "GEN_SQL_SCALAR",
    "SQL_AGGREGATE",
    "SQL_AGGREGATE_ADV",
    "SQL_AGGREGATE_NULL",
    "SQL_BLOB",
    "SQL_CAST",
    "SQL_DATETIME",
    "SQL_DDL",
    "SQL_DDL_DML",
    "SQL_DELETE",
    "SQL_DROP",
    "SQL_EXPRESSIONS",
    "SQL_FUNCTIONS",
    "SQL_INDEX",
    "SQL_INSERT",
    "SQL_JOIN",
    "SQL_JSON",
    "SQL_MATH",
    "SQL_NULL_ORDER",
    "SQL_NULL_SEMANTICS",
    "SQL_OPERATORS",
    "SQL_PATTERN",
    "SQL_ROWID",
    "SQL_SELECT",
    "SQL_STRING",
    "SQL_SUBQUERY",
    "SQL_TRANSACTION",
    "SQL_TYPE_AFFINITY",
    "SQL_UPDATE",
    "SQL_VALUES",
];

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub reference_bin: PathBuf,
    pub target_bin: PathBuf,
    pub output: PathBuf,
    pub tmp_root: PathBuf,
    pub workers: usize,
    pub repetitions: usize,
    pub warmup: usize,
    pub progress: bool,
    pub memory_samples: bool,
}

#[derive(Debug, Clone)]
pub struct RqlPhase1Case {
    pub reference: Case,
    pub target: Case,
}

#[derive(Debug, Clone)]
struct ShellOptions {
    mode: String,
    headers: Option<bool>,
    separator: Option<String>,
    null_value: Option<String>,
    row_separator: Option<String>,
}

impl Default for ShellOptions {
    fn default() -> Self {
        Self {
            mode: "list".to_owned(),
            headers: Some(false),
            separator: Some("|".to_owned()),
            null_value: Some("".to_owned()),
            row_separator: None,
        }
    }
}

pub fn rql_phase1_cases() -> Result<Vec<Case>> {
    let cases = super::catalog::selected_official_cases()?
        .into_iter()
        .filter(is_rql_phase1_source)
        .collect::<Vec<_>>();
    if cases.is_empty() {
        bail!("rql_phase1 selection matched zero cases");
    }
    Ok(cases)
}

pub fn run(config: RunConfig) -> Result<RunSummary> {
    let _workers = config.workers;
    let started = Instant::now();
    let reference = EngineSpec::new("sqlite3", config.reference_bin);
    let target = EngineSpec::new("redlinedb", config.target_bin);
    runner::validate_compare_engines(&reference, &target)?;

    let capabilities = reference.sqlite_shell_capabilities()?;
    let sqlite_version = capabilities
        .as_ref()
        .map(|capabilities| capabilities.version.clone());
    let target_capabilities = target.target_capabilities().ok();
    let partition = engine::partition_cases(
        rql_phase1_cases()?,
        capabilities.as_ref(),
        target_capabilities.as_ref(),
    );

    let mut summary = RunSummary::default();
    for skipped_case in partition.skipped {
        append_skip(
            &mut summary,
            &skipped_case.case,
            &skipped_case.reason,
            &reference,
            &target,
            sqlite_version.clone(),
            &config.output,
        )?;
    }

    let mut runnable = Vec::new();
    for case in partition.runnable {
        match rewrite_case(&case) {
            Ok(target_case) => runnable.push(RqlPhase1Case {
                reference: case,
                target: target_case,
            }),
            Err(err) => append_skip(
                &mut summary,
                &case,
                &format!("RQL phase-1 rewrite unsupported: {err}"),
                &reference,
                &target,
                sqlite_version.clone(),
                &config.output,
            )?,
        }
    }

    let total_samples = config.warmup.saturating_add(config.repetitions);
    for rql_case in runnable {
        if config.progress {
            eprintln!(
                "rql_phase1 case={} status=running",
                rql_case.reference.display_id()
            );
        }
        let mut failed = false;
        for sample_index in 0..total_samples {
            let measured_index = sample_index.checked_sub(config.warmup);
            let sample_role = if let Some(index) = measured_index {
                format!("measured:{}", index.saturating_add(1))
            } else {
                "warmup".to_owned()
            };
            let reference_output =
                reference.run_case(&rql_case.reference, &config.tmp_root, config.memory_samples)?;
            let target_output =
                target.run_case(&rql_case.target, &config.tmp_root, config.memory_samples)?;
            let status = validate_compare(&rql_case.reference, &reference_output, &target_output);
            let artifact = if let Err(reason) = &status {
                let artifact = report::write_failure_artifact(
                    &rql_case.reference,
                    &[&reference_output, &target_output],
                    &reason.to_string(),
                )?;
                eprintln!(
                    "rql_phase1 failure case={} reason={} artifact={}",
                    rql_case.reference.display_id(),
                    reason,
                    artifact.display()
                );
                Some(artifact)
            } else {
                None
            };
            report::append_jsonl(
                Some(&config.output),
                &report::compare_record(
                    &rql_case.reference,
                    &reference_output,
                    &target_output,
                    sample_index,
                    measured_index.map(|index| index.saturating_add(1)),
                    sample_role,
                    sqlite_version.clone(),
                    if status.is_ok() { "passed" } else { "failed" },
                    artifact,
                    status.as_ref().err().map(|reason| reason.to_string()),
                ),
            )?;
            if measured_index.is_some() {
                summary.slowest.push((
                    rql_case.reference.display_id(),
                    target_output.elapsed.as_nanos(),
                ));
            }
            if status.is_err() {
                failed = true;
            }
        }
        summary.total += 1;
        if failed {
            summary.failed += 1;
        } else {
            summary.passed += 1;
        }
        if config.progress {
            let status = if failed { "failed" } else { "passed" };
            eprintln!(
                "rql_phase1 case={} status={status}",
                rql_case.reference.display_id()
            );
        }
    }

    summary.elapsed = started.elapsed();
    finish_summary(summary, config.progress)
}

fn append_skip(
    summary: &mut RunSummary,
    case: &Case,
    reason: &str,
    reference: &EngineSpec,
    target: &EngineSpec,
    sqlite_version: Option<String>,
    output: &Path,
) -> Result<()> {
    summary.total += 1;
    summary.skipped += 1;
    let artifact = report::write_skip_artifact(case, reason)?;
    report::append_jsonl(
        Some(output),
        &report::skipped_compare_record(
            case,
            &reference.name,
            &target.name,
            sqlite_version,
            "skipped",
            Some(artifact),
            Some(reason.to_owned()),
        ),
    )
}

fn finish_summary(mut summary: RunSummary, progress: bool) -> Result<RunSummary> {
    summary.slowest.sort_by(|left, right| right.1.cmp(&left.1));
    summary.slowest.truncate(10);
    if progress {
        eprintln!(
            "rql_phase1 total={} passed={} failed={} skipped={} elapsed_ns={}",
            summary.total,
            summary.passed,
            summary.failed,
            summary.skipped,
            summary.elapsed.as_nanos()
        );
        eprintln!("rql_phase1 slowest={:?}", summary.slowest);
    }
    if summary.failed > 0 {
        bail!(
            "rql_phase1 failed {} of {} cases",
            summary.failed,
            summary.total
        );
    }
    Ok(summary)
}

fn is_rql_phase1_source(case: &Case) -> bool {
    RQL_PHASE1_CATEGORIES.contains(&case.category.as_str())
}

fn rewrite_case(case: &Case) -> Result<Case> {
    let program = rewrite_case_inner(case)?;
    let mut target = case.clone();
    target.kind = "rql_phase1".to_owned();
    target.stdin = format!("{}\n", serde_json::to_string_pretty(&program)?);
    target.args = target_args(case, &program.shell);
    Ok(target)
}

struct RqlRewrite {
    statements: JsonValue,
    shell: ShellOptions,
}

impl serde::Serialize for RqlRewrite {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        json!({ "statements": self.statements }).serialize(serializer)
    }
}

fn rewrite_case_inner(case: &Case) -> Result<RqlRewrite> {
    if case.expected_exit != 0 {
        bail!("expected-error cases are not performance candidates for RQL phase-1");
    }
    if known_runtime_divergence(case.id) {
        bail!("known RQL/SQLite runtime-output divergence for phase-1");
    }
    if !case.args.is_empty() || !case.files.is_empty() || case.script.is_some() {
        bail!("cases with args, files, or scripts are outside RQL phase-1");
    }
    let (sql, shell) = strip_dot_commands(&case.stdin)
        .with_context(|| format!("strip sqlite shell commands for {}", case.display_id()))?;
    let statements = Parser::parse_sql(&SQLiteDialect {}, &sql)
        .with_context(|| format!("parse SQL for {}", case.display_id()))?;
    if statements.is_empty() {
        bail!("case has no SQL statements");
    }
    let rql = statements
        .into_iter()
        .map(statement)
        .collect::<Result<Vec<_>>>()
        .with_context(|| format!("lower SQL to RQL for {}", case.display_id()))?;
    Ok(RqlRewrite {
        statements: JsonValue::Array(rql),
        shell,
    })
}

fn known_runtime_divergence(id: usize) -> bool {
    matches!(
        id,
        10456
            | 10514
            | 11001
            | 11002
            | 11003
            | 11005
            | 11006
            | 11007
            | 11008
            | 11010
            | 11011
            | 11012
            | 11014
            | 11015
            | 11016
            | 11018
            | 11019
            | 11021
            | 11022
            | 11023
            | 11033
            | 11035
            | 11037
            | 11038
            | 11039
            | 11041
            | 11042
            | 11043
            | 11045
            | 11046
            | 11047
            | 11053
            | 11054
            | 11055
            | 11065
            | 11066
            | 11067
            | 11070
            | 11073
            | 11074
            | 11075
            | 11081
            | 11082
            | 11083
            | 11085
            | 11086
            | 11087
            | 11092
            | 11093
            | 11095
            | 11096
            | 11209
            | 11214
            | 11219
            | 11224
            | 11229
            | 11234
            | 11239
            | 11244
            | 11306
            | 11314
            | 11317
            | 11322
            | 11325
            | 11330
            | 11338
            | 11346
            | 11354
            | 11362
            | 11410
            | 11436
            | 11437
            | 11438
            | 11439
            | 11523
            | 11528
            | 11578
            | 11583
            | 11633
            | 11638
    )
}

fn target_args(case: &Case, shell: &ShellOptions) -> Vec<String> {
    let mut args = vec!["--batch".to_owned(), "--bail".to_owned()];
    args.push(format!("--{}", shell.mode));
    match shell.headers {
        Some(true) => args.push("--header".to_owned()),
        Some(false) => args.push("--noheader".to_owned()),
        None => {}
    }
    if let Some(separator) = &shell.separator {
        args.push("--separator".to_owned());
        args.push(separator.clone());
    }
    if let Some(null_value) = &shell.null_value {
        args.push("--nullvalue".to_owned());
        args.push(null_value.clone());
    }
    if let Some(row_separator) = &shell.row_separator {
        args.push("--newline".to_owned());
        args.push(row_separator.clone());
    }
    args.push("--rql".to_owned());
    args.push(case.db.clone());
    args
}

fn strip_dot_commands(input: &str) -> Result<(String, ShellOptions)> {
    let mut shell = ShellOptions::default();
    let mut sql = String::new();
    let mut saw_sql = false;
    for line in input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('.') {
            if saw_sql {
                bail!("dot-command after SQL is not representable as static RQL CLI flags");
            }
            apply_dot_command(trimmed, &mut shell)?;
            continue;
        }
        if !trimmed.is_empty() && !trimmed.starts_with("--") {
            saw_sql = true;
        }
        sql.push_str(line);
        sql.push('\n');
    }
    Ok((sql, shell))
}

fn apply_dot_command(line: &str, shell: &mut ShellOptions) -> Result<()> {
    let parts = split_dot_args(line)?;
    let Some(command) = parts.first().map(String::as_str) else {
        return Ok(());
    };
    match command {
        ".mode" => {
            let mode = parts.get(1).map(String::as_str).unwrap_or("list");
            match mode {
                "ascii" | "box" | "column" | "csv" | "html" | "json" | "line" | "list"
                | "markdown" | "quote" | "table" | "tabs" | "tcl" => {
                    shell.mode = mode.to_owned();
                    shell.headers = None;
                    shell.separator = None;
                    shell.null_value = None;
                    shell.row_separator = None;
                }
                other => bail!("unsupported .mode `{other}`"),
            }
        }
        ".headers" | ".header" => {
            let value = parts.get(1).map(String::as_str).unwrap_or("on");
            shell.headers = Some(parse_on_off(value)?);
        }
        ".separator" => {
            shell.separator = Some(parts.get(1).cloned().unwrap_or_else(|| "|".to_owned()));
        }
        ".nullvalue" => {
            shell.null_value = Some(parts.get(1).cloned().unwrap_or_default());
        }
        ".newline" => {
            shell.row_separator = Some(parts.get(1).cloned().unwrap_or_else(|| "\n".to_owned()));
        }
        ".bail" | ".echo" | ".timer" => {}
        other => bail!("unsupported dot-command `{other}`"),
    }
    Ok(())
}

fn split_dot_args(line: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote = None::<char>;
    let mut chars = line.trim().chars().peekable();
    while let Some(ch) = chars.next() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }
    if quote.is_some() {
        bail!("unterminated quote in dot-command `{line}`");
    }
    if !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

fn parse_on_off(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "yes" | "true" | "1" => Ok(true),
        "off" | "no" | "false" | "0" => Ok(false),
        other => bail!("expected on/off, got `{other}`"),
    }
}

fn statement(stmt: Statement) -> Result<JsonValue> {
    Ok(match stmt {
        Statement::Query(query) => json!({
            "type": "select",
            "select": null_to_absent(select_query(*query)?)
        })
        .get("select")
        .cloned()
        .unwrap(),
        Statement::CreateTable(create) => create_table(create)?,
        Statement::CreateIndex(index) => create_index(index)?,
        Statement::Drop {
            object_type,
            if_exists,
            names,
            ..
        } => drop_statement(object_type, if_exists, names)?,
        Statement::Insert(insert) => insert_statement(insert)?,
        Statement::Update(update) => update_statement(update)?,
        Statement::Delete(delete) => delete_statement(delete)?,
        Statement::StartTransaction { .. } => json!({
            "type": "begin",
            "mode": "deferred"
        }),
        Statement::Commit { .. } => json!({ "type": "commit" }),
        Statement::Rollback {
            savepoint: None, ..
        } => json!({ "type": "rollback" }),
        Statement::Rollback {
            savepoint: Some(_), ..
        }
        | Statement::Savepoint { .. }
        | Statement::ReleaseSavepoint { .. } => bail!("RQL v0.1 does not expose savepoints"),
        other => bail!("unsupported statement `{other}`"),
    })
}

fn null_to_absent(value: JsonValue) -> JsonValue {
    value
}

fn create_table(create: sqlparser::ast::CreateTable) -> Result<JsonValue> {
    if create.query.is_some() {
        bail!("CREATE TABLE AS SELECT is outside RQL phase-1 JSON lowering");
    }
    if create.temporary {
        bail!("temporary tables are outside RQL phase-1");
    }
    if !create.constraints.is_empty() {
        bail!("table constraints are outside RQL phase-1");
    }
    let columns = create
        .columns
        .into_iter()
        .map(column_def)
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "type": "create_table",
        "table": rql_name(create.name)?,
        "if_not_exists": create.if_not_exists,
        "columns": columns,
        "strict": create.strict,
        "without_rowid": create.without_rowid
    }))
}

fn column_def(column: sqlparser::ast::ColumnDef) -> Result<JsonValue> {
    let mut primary_key = false;
    let mut not_null = false;
    let mut unique = false;
    let mut default = None;
    for option in column.options {
        match option.option {
            ColumnOption::PrimaryKey(_) => primary_key = true,
            ColumnOption::NotNull => not_null = true,
            ColumnOption::Unique(_) => unique = true,
            ColumnOption::Default(expr) => default = Some(literal_expr(expr)?),
            ColumnOption::Null | ColumnOption::DialectSpecific(_) => {}
            ColumnOption::Check(_) => bail!("CHECK constraints are outside RQL phase-1"),
            ColumnOption::Generated { .. } => bail!("generated columns are outside RQL phase-1"),
            ColumnOption::ForeignKey(_) => bail!("foreign keys are outside RQL phase-1"),
            other => bail!("unsupported column option `{other}`"),
        }
    }
    Ok(json!({
        "name": column.name.value,
        "declared_type": declared_type(column.data_type),
        "primary_key": primary_key,
        "not_null": not_null,
        "unique": unique,
        "default": default
    }))
}

fn declared_type(data_type: DataType) -> Option<String> {
    if data_type == DataType::Unspecified {
        None
    } else {
        Some(data_type.to_string())
    }
}

fn literal_expr(expr: Expr) -> Result<JsonValue> {
    match expr {
        Expr::Value(value) => literal_value(value.value),
        other => bail!("default expression `{other}` is not a literal"),
    }
}

fn create_index(index: sqlparser::ast::CreateIndex) -> Result<JsonValue> {
    if index.name.is_none() {
        bail!("RQL CREATE INDEX requires an index name");
    }
    if index.predicate.is_some()
        || !index.include.is_empty()
        || index.nulls_distinct.is_some()
        || !index.with.is_empty()
    {
        bail!("partial or extended index syntax is outside RQL phase-1");
    }
    let columns = index
        .columns
        .into_iter()
        .map(|column| {
            if column.operator_class.is_some() {
                bail!("index operator classes are outside RQL phase-1");
            }
            let name = match column.column.expr {
                Expr::Identifier(ident) => ident.value,
                other => bail!("index expression `{other}` is outside RQL phase-1"),
            };
            if column.column.options.nulls_first.is_some() {
                bail!("index NULLS ordering is outside RQL phase-1");
            }
            Ok(json!({
                "name": name,
                "descending": column.column.options.asc == Some(false)
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "type": "create_index",
        "index": rql_name(index.name.unwrap())?,
        "table": rql_name(index.table_name)?,
        "if_not_exists": index.if_not_exists,
        "unique": index.unique,
        "columns": columns
    }))
}

fn drop_statement(
    object_type: ObjectType,
    if_exists: bool,
    names: Vec<ObjectName>,
) -> Result<JsonValue> {
    if names.len() != 1 {
        bail!("RQL DROP supports one object at a time");
    }
    let name = rql_name(names.into_iter().next().unwrap())?;
    match object_type {
        ObjectType::Table => Ok(json!({
            "type": "drop_table",
            "table": name,
            "if_exists": if_exists
        })),
        ObjectType::Index => Ok(json!({
            "type": "drop_index",
            "index": name,
            "if_exists": if_exists
        })),
        other => bail!("RQL DROP does not support {other}"),
    }
}

fn insert_statement(insert: sqlparser::ast::Insert) -> Result<JsonValue> {
    if insert.or.is_some() || insert.on.is_some() || insert.replace_into || insert.ignore {
        bail!("INSERT conflict handling is outside RQL phase-1");
    }
    if insert.returning.is_some() {
        bail!("INSERT RETURNING is outside RQL phase-1");
    }
    if !insert.assignments.is_empty() {
        bail!("INSERT SET is outside RQL phase-1");
    }
    let table = match insert.table {
        sqlparser::ast::TableObject::TableName(name) => rql_name(name)?,
        other => bail!("unsupported INSERT target `{other}`"),
    };
    let columns = insert
        .columns
        .into_iter()
        .map(|column| column.value)
        .collect::<Vec<_>>();
    let Some(source) = insert.source else {
        return Ok(json!({
            "type": "insert",
            "table": table,
            "columns": columns,
            "default_values": true
        }));
    };
    match *source.body {
        SetExpr::Values(values) => {
            let rows = values
                .rows
                .into_iter()
                .map(|row| row.into_iter().map(expr).collect::<Result<Vec<_>>>())
                .collect::<Result<Vec<_>>>()?;
            Ok(json!({
                "type": "insert",
                "table": table,
                "columns": columns,
                "values": rows
            }))
        }
        _ => bail!("INSERT SELECT is outside RQL phase-1 JSON lowering"),
    }
}

fn update_statement(update: sqlparser::ast::Update) -> Result<JsonValue> {
    if update.or.is_some() || update.from.is_some() || update.limit.is_some() {
        bail!("extended UPDATE syntax is outside RQL phase-1");
    }
    if update.returning.is_some() {
        bail!("UPDATE RETURNING is outside RQL phase-1");
    }
    if !update.table.joins.is_empty() {
        bail!("UPDATE joins are outside RQL phase-1");
    }
    let table = match update.table.relation {
        TableFactor::Table { name, alias, .. } => {
            if alias.is_some() {
                bail!("UPDATE aliases are outside RQL phase-1");
            }
            rql_name(name)?
        }
        other => bail!("unsupported UPDATE table `{other}`"),
    };
    let assignments = update
        .assignments
        .into_iter()
        .map(|assignment| {
            let column = match assignment.target {
                AssignmentTarget::ColumnName(name) => object_name_leaf(name)?,
                AssignmentTarget::Tuple(_) => bail!("tuple assignment is outside RQL phase-1"),
            };
            Ok(json!({
                "column": column,
                "value": expr(assignment.value)?
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "type": "update",
        "table": table,
        "assignments": assignments,
        "filter": update.selection.map(expr).transpose()?
    }))
}

fn delete_statement(delete: sqlparser::ast::Delete) -> Result<JsonValue> {
    if delete.using.is_some() || !delete.order_by.is_empty() || delete.limit.is_some() {
        bail!("extended DELETE syntax is outside RQL phase-1");
    }
    if delete.returning.is_some() {
        bail!("DELETE RETURNING is outside RQL phase-1");
    }
    let from = match delete.from {
        sqlparser::ast::FromTable::WithFromKeyword(from)
        | sqlparser::ast::FromTable::WithoutKeyword(from) => from,
    };
    if from.len() != 1 || !from[0].joins.is_empty() {
        bail!("RQL DELETE supports a single table");
    }
    let table = match &from[0].relation {
        TableFactor::Table { name, alias, .. } => {
            if alias.is_some() {
                bail!("DELETE aliases are outside RQL phase-1");
            }
            rql_name(name.clone())?
        }
        other => bail!("unsupported DELETE table `{other}`"),
    };
    Ok(json!({
        "type": "delete",
        "table": table,
        "filter": delete.selection.map(expr).transpose()?
    }))
}

fn select_query(query: Query) -> Result<JsonValue> {
    if query.with.is_some()
        || query.fetch.is_some()
        || !query.locks.is_empty()
        || query.for_clause.is_some()
        || query.settings.is_some()
        || query.format_clause.is_some()
        || !query.pipe_operators.is_empty()
    {
        bail!("extended SELECT query syntax is outside RQL phase-1");
    }
    let mut select = match *query.body {
        SetExpr::Select(select) => select_statement(*select)?,
        other => bail!("query body `{other}` is outside RQL phase-1"),
    };
    if let Some(order_by) = query.order_by {
        match order_by.kind {
            OrderByKind::Expressions(orders) => {
                select["order_by"] = JsonValue::Array(
                    orders
                        .into_iter()
                        .map(|order| {
                            Ok(json!({
                                "expr": expr(order.expr)?,
                                "descending": order.options.asc == Some(false),
                                "nulls_first": order.options.nulls_first
                            }))
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
            }
            _ => bail!("ORDER BY ALL is outside RQL phase-1"),
        }
    }
    if let Some(limit_clause) = query.limit_clause {
        match limit_clause {
            LimitClause::LimitOffset { limit, offset, .. } => {
                if let Some(limit) = limit {
                    select["limit"] = json!(u64_literal_expr(limit)?);
                }
                if let Some(offset) = offset {
                    select["offset"] = json!(u64_literal_expr(offset.value)?);
                }
            }
            other => bail!("limit clause `{other}` is outside RQL phase-1"),
        }
    }
    Ok(select)
}

fn select_statement(select: sqlparser::ast::Select) -> Result<JsonValue> {
    if select.top.is_some()
        || select.into.is_some()
        || !select.lateral_views.is_empty()
        || select.prewhere.is_some()
        || !select.connect_by.is_empty()
        || !select.cluster_by.is_empty()
        || !select.distribute_by.is_empty()
        || !select.sort_by.is_empty()
        || !select.named_window.is_empty()
        || select.qualify.is_some()
        || select.value_table_mode.is_some()
    {
        bail!("extended SELECT syntax is outside RQL phase-1");
    }
    let distinct = match select.distinct {
        None | Some(Distinct::All) => false,
        Some(Distinct::Distinct) => true,
        Some(Distinct::On(_)) => bail!("DISTINCT ON is outside RQL phase-1"),
    };
    let projection = select
        .projection
        .into_iter()
        .map(select_item)
        .collect::<Result<Vec<_>>>()?;
    let (from, joins) = from_tables(select.from)?;
    let group_by = match select.group_by {
        sqlparser::ast::GroupByExpr::Expressions(exprs, modifiers) => {
            if !modifiers.is_empty() {
                bail!("GROUP BY modifiers are outside RQL phase-1");
            }
            exprs.into_iter().map(expr).collect::<Result<Vec<_>>>()?
        }
        sqlparser::ast::GroupByExpr::All(_) => bail!("GROUP BY ALL is outside RQL phase-1"),
    };
    Ok(json!({
        "type": "select",
        "distinct": distinct,
        "projection": projection,
        "from": from,
        "joins": joins,
        "filter": select.selection.map(expr).transpose()?,
        "group_by": group_by,
        "having": select.having.map(expr).transpose()?
    }))
}

fn select_item(item: SelectItem) -> Result<JsonValue> {
    match item {
        SelectItem::Wildcard(_) => Ok(json!({ "type": "wildcard" })),
        SelectItem::QualifiedWildcard(kind, _) => Ok(json!({
            "type": "qualified_wildcard",
            "table": kind.to_string().trim_end_matches(".*")
        })),
        SelectItem::UnnamedExpr(value) => Ok(json!({
            "type": "expr",
            "expr": expr(value)?
        })),
        SelectItem::ExprWithAlias { expr: value, alias } => Ok(json!({
            "type": "expr",
            "expr": expr(value)?,
            "alias": alias.value
        })),
    }
}

fn from_tables(tables: Vec<TableWithJoins>) -> Result<(Option<JsonValue>, Vec<JsonValue>)> {
    if tables.is_empty() {
        return Ok((None, Vec::new()));
    }
    let mut iter = tables.into_iter();
    let first = iter.next().unwrap();
    let from = Some(table_factor(first.relation)?);
    let mut joins = first
        .joins
        .into_iter()
        .map(join)
        .collect::<Result<Vec<_>>>()?;
    for table in iter {
        if !table.joins.is_empty() {
            bail!("comma joins with nested joins are outside RQL phase-1");
        }
        joins.push(json!({
            "table": table_factor(table.relation)?,
            "kind": "cross"
        }));
    }
    Ok((from, joins))
}

fn join(join: sqlparser::ast::Join) -> Result<JsonValue> {
    let table = table_factor(join.relation)?;
    let (kind, constraint) = match join.join_operator {
        JoinOperator::Join(constraint) | JoinOperator::Inner(constraint) => ("inner", constraint),
        JoinOperator::Left(constraint) | JoinOperator::LeftOuter(constraint) => {
            ("left", constraint)
        }
        JoinOperator::Right(constraint) | JoinOperator::RightOuter(constraint) => {
            ("right", constraint)
        }
        JoinOperator::FullOuter(constraint) => ("full", constraint),
        JoinOperator::CrossJoin(constraint) => ("cross", constraint),
        other => bail!("join operator `{:?}` is outside RQL phase-1", other),
    };
    let on = match constraint {
        JoinConstraint::On(value) => Some(expr(value)?),
        JoinConstraint::None => None,
        other => bail!("join constraint `{:?}` is outside RQL phase-1", other),
    };
    Ok(json!({
        "table": table,
        "kind": kind,
        "on": on
    }))
}

fn table_factor(factor: TableFactor) -> Result<JsonValue> {
    match factor {
        TableFactor::Table {
            name,
            alias,
            args,
            with_hints,
            version,
            with_ordinality,
            partitions,
            json_path,
            sample,
            index_hints,
            ..
        } => {
            if args.is_some()
                || !with_hints.is_empty()
                || version.is_some()
                || with_ordinality
                || !partitions.is_empty()
                || json_path.is_some()
                || sample.is_some()
                || !index_hints.is_empty()
            {
                bail!("extended table factor syntax is outside RQL phase-1");
            }
            Ok(json!({
                "name": rql_name(name)?,
                "alias": alias_name(alias)
            }))
        }
        other => bail!("table factor `{other}` is outside RQL phase-1"),
    }
}

fn alias_name(alias: Option<TableAlias>) -> Option<String> {
    alias.map(|alias| alias.name.value)
}

fn expr(value: Expr) -> Result<JsonValue> {
    Ok(match value {
        Expr::Identifier(ident) => json!({
            "type": "column",
            "column": { "name": ident.value }
        }),
        Expr::CompoundIdentifier(parts) => compound_column(parts)?,
        Expr::Value(value) => literal_value(value.value)?,
        Expr::UnaryOp { op, expr: inner } => json!({
            "type": "unary",
            "op": unary_op(op)?,
            "expr": expr(*inner)?
        }),
        Expr::BinaryOp { left, op, right } => json!({
            "type": "binary",
            "left": expr(*left)?,
            "op": binary_op(op)?,
            "right": expr(*right)?
        }),
        Expr::Like {
            negated,
            any,
            expr: inner,
            pattern,
            escape_char,
        } => {
            if any || escape_char.is_some() {
                bail!("LIKE ANY/ESCAPE is outside RQL phase-1");
            }
            json!({
                "type": "binary",
                "left": expr(*inner)?,
                "op": if negated { "not_like" } else { "like" },
                "right": expr(*pattern)?
            })
        }
        Expr::Nested(inner) => json!({
            "type": "nested",
            "expr": expr(*inner)?
        }),
        Expr::Function(function) => function_expr(function)?,
        Expr::Cast {
            expr: inner,
            data_type,
            ..
        } => json!({
            "type": "cast",
            "expr": expr(*inner)?,
            "data_type": data_type.to_string()
        }),
        Expr::IsNull(inner) => json!({
            "type": "is_null",
            "expr": expr(*inner)?
        }),
        Expr::IsNotNull(inner) => json!({
            "type": "is_null",
            "expr": expr(*inner)?,
            "negated": true
        }),
        Expr::Between {
            expr: inner,
            negated,
            low,
            high,
        } => json!({
            "type": "between",
            "expr": expr(*inner)?,
            "low": expr(*low)?,
            "high": expr(*high)?,
            "negated": negated
        }),
        Expr::InList {
            expr: inner,
            list,
            negated,
        } => json!({
            "type": "in_list",
            "expr": expr(*inner)?,
            "list": list.into_iter().map(expr).collect::<Result<Vec<_>>>()?,
            "negated": negated
        }),
        Expr::InSubquery {
            expr: inner,
            subquery,
            negated,
        } => json!({
            "type": "in_subquery",
            "expr": expr(*inner)?,
            "select": select_query(*subquery)?,
            "negated": negated
        }),
        Expr::Subquery(query) => json!({
            "type": "subquery",
            "select": select_query(*query)?
        }),
        Expr::Exists { subquery, negated } => json!({
            "type": "exists",
            "select": select_query(*subquery)?,
            "negated": negated
        }),
        other => bail!("expression `{other}` is outside RQL phase-1"),
    })
}

fn function_expr(function: sqlparser::ast::Function) -> Result<JsonValue> {
    if function.filter.is_some()
        || function.null_treatment.is_some()
        || function.over.is_some()
        || !function.within_group.is_empty()
        || !matches!(function.parameters, FunctionArguments::None)
    {
        bail!("extended function syntax is outside RQL phase-1");
    }
    let (args, distinct) = match function.args {
        FunctionArguments::None => (Vec::new(), false),
        FunctionArguments::Subquery(_) => bail!("function subquery args are outside RQL phase-1"),
        FunctionArguments::List(list) => {
            if !list.clauses.is_empty() {
                bail!("function argument clauses are outside RQL phase-1");
            }
            let distinct = matches!(list.duplicate_treatment, Some(DuplicateTreatment::Distinct));
            let mut args = Vec::new();
            for arg in list.args {
                match arg {
                    FunctionArg::Unnamed(FunctionArgExpr::Expr(value)) => args.push(expr(value)?),
                    FunctionArg::Unnamed(FunctionArgExpr::Wildcard) => {
                        if function.name.to_string().eq_ignore_ascii_case("count") {
                            return Ok(json!({ "type": "count_star" }));
                        }
                        bail!("wildcard function args are supported only for count(*)");
                    }
                    other => bail!("function argument `{other}` is outside RQL phase-1"),
                }
            }
            (args, distinct)
        }
    };
    Ok(json!({
        "type": "function",
        "name": function.name.to_string(),
        "args": args,
        "distinct": distinct
    }))
}

fn literal_value(value: Value) -> Result<JsonValue> {
    Ok(match value {
        Value::Null => json!({ "type": "null" }),
        Value::Boolean(value) => json!({ "type": "bool", "value": value }),
        Value::Number(raw, _) => number_literal(&raw)?,
        Value::HexStringLiteral(raw) => json!({
            "type": "blob",
            "bytes": hex_bytes(&raw)?
        }),
        Value::Placeholder(raw) => {
            let index = raw.trim_start_matches('?').parse::<usize>()?;
            json!({ "type": "param", "index": index })
        }
        other => {
            let display = other.to_string();
            if let Some(value) = other.into_string() {
                json!({ "type": "text", "value": value })
            } else {
                bail!("literal `{display}` is outside RQL phase-1");
            }
        }
    })
}

fn number_literal(raw: &str) -> Result<JsonValue> {
    if raw.contains('.') || raw.contains('e') || raw.contains('E') {
        Ok(json!({ "type": "real", "value": raw.parse::<f64>()? }))
    } else {
        Ok(json!({ "type": "integer", "value": raw.parse::<i64>()? }))
    }
}

fn hex_bytes(raw: &str) -> Result<Vec<u8>> {
    let clean = raw.trim();
    if clean.len() % 2 != 0 {
        bail!("odd-length hex blob literal");
    }
    (0..clean.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&clean[idx..idx + 2], 16).map_err(Into::into))
        .collect()
}

fn compound_column(parts: Vec<Ident>) -> Result<JsonValue> {
    match parts.as_slice() {
        [name] => Ok(json!({
            "type": "column",
            "column": { "name": name.value }
        })),
        [table, name] => Ok(json!({
            "type": "column",
            "column": { "table": table.value, "name": name.value }
        })),
        [schema, table, name] if schema.value.eq_ignore_ascii_case("main") => Ok(json!({
            "type": "column",
            "column": { "table": table.value, "name": name.value }
        })),
        _ => bail!("compound identifier is outside RQL phase-1"),
    }
}

fn unary_op(op: UnaryOperator) -> Result<&'static str> {
    match op {
        UnaryOperator::Not | UnaryOperator::BangNot => Ok("not"),
        UnaryOperator::Minus => Ok("negate"),
        UnaryOperator::Plus => Ok("positive"),
        UnaryOperator::BitwiseNot => Ok("bit_not"),
        other => bail!("unary operator `{other}` is outside RQL phase-1"),
    }
}

fn binary_op(op: BinaryOperator) -> Result<&'static str> {
    match op {
        BinaryOperator::Eq => Ok("eq"),
        BinaryOperator::NotEq => Ok("not_eq"),
        BinaryOperator::Lt => Ok("lt"),
        BinaryOperator::LtEq => Ok("lt_eq"),
        BinaryOperator::Gt => Ok("gt"),
        BinaryOperator::GtEq => Ok("gt_eq"),
        BinaryOperator::And => Ok("and"),
        BinaryOperator::Or => Ok("or"),
        BinaryOperator::Plus => Ok("add"),
        BinaryOperator::Minus => Ok("sub"),
        BinaryOperator::Multiply => Ok("mul"),
        BinaryOperator::Divide => Ok("div"),
        BinaryOperator::Modulo => Ok("mod"),
        BinaryOperator::StringConcat => Ok("concat"),
        other => bail!("binary operator `{other}` is outside RQL phase-1"),
    }
}

fn u64_literal_expr(value: Expr) -> Result<u64> {
    match value {
        Expr::Value(value) => match value.value {
            Value::Number(raw, _) => Ok(raw.parse::<u64>()?),
            other => bail!("LIMIT/OFFSET literal `{other}` is outside RQL phase-1"),
        },
        other => bail!("LIMIT/OFFSET expression `{other}` is outside RQL phase-1"),
    }
}

fn rql_name(name: ObjectName) -> Result<JsonValue> {
    match name.0.as_slice() {
        [ObjectNamePart::Identifier(name)] => Ok(json!({ "name": name.value })),
        [
            ObjectNamePart::Identifier(schema),
            ObjectNamePart::Identifier(name),
        ] => Ok(json!({
            "schema": schema.value,
            "name": name.value
        })),
        _ => bail!("object name `{name}` is outside RQL phase-1"),
    }
}

fn object_name_leaf(name: ObjectName) -> Result<String> {
    match name.0.as_slice() {
        [ObjectNamePart::Identifier(name)] => Ok(name.value.clone()),
        _ => bail!("qualified assignment target `{name}` is outside RQL phase-1"),
    }
}

fn validate_compare(case: &Case, reference: &EngineOutput, target: &EngineOutput) -> Result<()> {
    if reference.status_code != target.status_code {
        bail!(
            "exit mismatch: reference {:?}, target {:?}",
            reference.status_code,
            target.status_code
        );
    }
    if !case.compare_stdout {
        return Ok(());
    }
    let reference_stdout = normalize_compare_output(case, reference, &reference.stdout);
    let target_stdout = normalize_compare_output(case, target, &target.stdout);
    if reference_stdout != target_stdout {
        bail!("stdout mismatch: reference `{reference_stdout}`, target `{target_stdout}`");
    }
    if reference.status_code != Some(0) || case.status == "catalog_only" {
        return Ok(());
    }
    let reference_stderr = normalize_compare_output(case, reference, &reference.stderr);
    let target_stderr = normalize_compare_output(case, target, &target.stderr);
    if reference_stderr != target_stderr {
        bail!("stderr mismatch: reference `{reference_stderr}`, target `{target_stderr}`");
    }
    Ok(())
}

fn normalize_compare_output(case: &Case, output: &EngineOutput, value: &str) -> String {
    let normalized = normalize_output(value);
    let marker = format!(
        "/{}-{}-{}",
        case.display_id(),
        sanitize_identifier(&output.engine),
        std::process::id()
    );
    normalized.replace(&marker, "/{{CASE_TMP}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_parity::case::Priority;

    #[test]
    fn rql_phase1_candidate_counts_match_plan() {
        let cases = rql_phase1_cases().expect("rql phase1 cases");
        assert_eq!(cases.len(), 1385);
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.priority == Priority::P0)
                .count(),
            736
        );
    }

    #[test]
    fn rql_phase1_rewrites_basic_create_insert_select() {
        let case = rql_phase1_cases()
            .expect("rql phase1 cases")
            .into_iter()
            .find(|case| case.id == 3)
            .expect("case 3");
        let target = rewrite_case(&case).expect("rewrite");
        assert!(target.args.iter().any(|arg| arg == "--rql"));
        assert!(target.stdin.contains("\"type\": \"create_table\""));
        assert!(target.stdin.contains("\"type\": \"insert\""));
        assert!(target.stdin.contains("\"type\": \"select\""));
    }
}
