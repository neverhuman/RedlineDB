use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

thread_local! {
    /// WS-A2f-rewrite: opt-in flag toggled by
    /// `PRAGMA redline_dml_order_limit_rewrite = ON`. When set, the pre-parse
    /// rewriter lowers `DELETE/UPDATE ... [WHERE ...] [ORDER BY ...] LIMIT n
    /// [OFFSET m]` into the equivalent `... WHERE rowid IN (SELECT rowid
    /// FROM t ... ORDER BY ... LIMIT n OFFSET m)` subquery form so the
    /// rejected SQLite-compat syntax becomes executable without changing
    /// the executor or breaking parity case 00220 (which expects the
    /// reference-style rejection at the default OFF setting).
    static DML_ORDER_LIMIT_REWRITE_ENABLED: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn dml_order_limit_rewrite_enabled() -> bool {
    DML_ORDER_LIMIT_REWRITE_ENABLED.with(|c| c.get())
}

pub(crate) fn set_dml_order_limit_rewrite_enabled(value: bool) {
    DML_ORDER_LIMIT_REWRITE_ENABLED.with(|c| c.set(value));
}

#[allow(unused_imports)]
use redlinedb_kernel::catalog::{
    ColumnConstraintSpec, ColumnSpec, ConflictAction, CreateIndexSpec, CreateTableSpec,
    CreateTriggerSpec, CreateViewSpec, DbName, DropIndexSpec, DropTableSpec, DropTriggerSpec,
    DropViewSpec, ExprAst, IndexColumnSpec, IndexOrigin, OwnedValue, QualifiedName, SchemaEpoch,
    SchemaSnapshot, SortDir, TableConstraintSpec, TriggerEventKind, TriggerTimeKind, lookup_index,
    lookup_table,
};
#[allow(unused_imports)]
use sqlparser::ast::{
    AlterTableOperation, Analyze as SqlAnalyze, AnalyzeFormat, AnalyzeFormatKind, BinaryOperator,
    ColumnDef, ColumnOption, ConflictTarget, Distinct, Expr, FunctionArg, FunctionArgExpr,
    FunctionArgumentClause, FunctionArguments, GroupByExpr, Ident, IndexColumn, JoinConstraint,
    JoinOperator, LimitClause, ObjectName, ObjectNamePart, OnConflictAction, OnInsert, OrderByExpr,
    OrderByKind, Query, SelectItem, SetExpr, SetOperator, SetQuantifier, SqliteOnConflict,
    Statement as SqlStatement, TableFactor, TableObject, TableWithJoins, UnaryOperator, Value,
    ValueWithSpan,
};
#[allow(unused_imports)]
use sqlparser::dialect::SQLiteDialect;
#[allow(unused_imports)]
use sqlparser::parser::Parser;

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::session::BeginMode;
#[allow(unused_imports)]
use crate::statement::*;
use crate::value::SqlValue;

pub(crate) mod bind;
mod helpers;
#[allow(unused_imports)]
pub(crate) use helpers::*;
mod ddl;
#[allow(unused_imports)]
pub(crate) use ddl::*;
mod dml;
#[allow(unused_imports)]
pub(crate) use dml::*;
pub(crate) mod pragma;
#[allow(unused_imports)]
pub(crate) use pragma::*;
mod pragma_compile;
#[allow(unused_imports)]
pub(crate) use pragma_compile::*;
mod prepare;
pub(crate) mod savepoint;
mod select;
#[allow(unused_imports)]
pub(crate) use select::*;
mod rewrite;
mod split;
mod templates;
#[allow(unused_imports)]
pub(crate) use rewrite::*;
pub use split::{first_statement_complete, is_blank_sql, split_first_statement, split_statements};
pub(crate) use templates::{bind_statement, template};

pub(crate) fn is_pragma_sql(sql: &str) -> bool {
    // A31: byte-wise case-insensitive prefix check. The previous
    // implementation called `to_ascii_lowercase()` which heap-allocates
    // the entire trimmed SQL just to call `.starts_with("pragma")`. This
    // is invoked once per `prepare_cached_inner` (i.e. per statement
    // preparation), so the alloc is on the hot path for every prepared
    // template — wasted on the non-PRAGMA majority.
    const PRAGMA: &[u8] = b"pragma";
    let bytes = sql
        .trim_start()
        .trim_end_matches(';')
        .trim_start()
        .as_bytes();
    if bytes.len() < PRAGMA.len() {
        return false;
    }
    bytes[..PRAGMA.len()]
        .iter()
        .zip(PRAGMA.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

pub fn parse_prepared_template(conn: &Connection, sql: &str) -> Result<PreparedTemplate> {
    match catch_unwind(AssertUnwindSafe(|| parse_prepared_template_impl(conn, sql))) {
        Ok(result) => result,
        Err(payload) => Err(Error::Parse(format!(
            "sql parser panic: {}",
            panic_payload_to_string(payload)
        ))),
    }
}

fn parse_prepared_template_impl(conn: &Connection, sql: &str) -> Result<PreparedTemplate> {
    let trimmed = sql.trim();
    let stmt = trimmed.trim_end_matches(';').trim();
    let schema = conn.schema_snapshot();
    let schema_epoch = conn.schema_epoch();

    // Track J: strip Postgres-registered schema prefixes (`sch.t` → `t`)
    // before further parsing. SQLite has no schema layer; the kernel rejects
    // any qualifier other than `main`, so once the session has registered
    // the namespace via CREATE SCHEMA we treat qualified references as
    // ordinary table names in the main schema.
    if let Some(rewritten) = strip_registered_pg_schema_prefixes(conn, sql) {
        if rewritten != sql {
            return parse_prepared_template_impl(conn, &rewritten);
        }
    }
    // Track J: rewrite SELECTs against `pg_namespace` / `pg_class` into a
    // session-snapshotted VALUES list so the introspection probes that the
    // beyond-pg parity gates use see the expected names back.
    if let Some(rewritten) = rewrite_pg_catalog_query(conn, sql) {
        return parse_prepared_template_impl(conn, &rewritten);
    }
    // Track J: strip Postgres `::regclass` and similar cast suffixes that
    // RedlineDB has no need to evaluate; the wrapped string is the natural
    // identifier the parity probes care about.
    if let Some(rewritten) = strip_pg_cast_suffixes(sql) {
        return parse_prepared_template_impl(conn, &rewritten);
    }

    // Phase 1.2 fast-paths: each `==` here was previously a comparison
    // against a full-string `to_ascii_lowercase()` allocation of the SQL.
    // `eq_ignore_ascii_case` does the byte-folding inline with no heap.
    if stmt.eq_ignore_ascii_case("begin")
        || stmt.eq_ignore_ascii_case("begin transaction")
        || stmt.eq_ignore_ascii_case("begin deferred")
    {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Deferred),
        ));
    }
    if stmt.eq_ignore_ascii_case("begin immediate")
        || stmt.eq_ignore_ascii_case("begin immediate transaction")
    {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Immediate),
        ));
    }
    if stmt.eq_ignore_ascii_case("begin exclusive")
        || stmt.eq_ignore_ascii_case("begin exclusive transaction")
    {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Begin(BeginMode::Exclusive),
        ));
    }
    if stmt.eq_ignore_ascii_case("commit")
        || stmt.eq_ignore_ascii_case("commit transaction")
        || stmt.eq_ignore_ascii_case("end")
        || stmt.eq_ignore_ascii_case("end transaction")
    {
        return Ok(template(trimmed, schema_epoch, false, PreparedKind::Commit));
    }
    if stmt.eq_ignore_ascii_case("rollback") || stmt.eq_ignore_ascii_case("rollback transaction") {
        return Ok(template(
            trimmed,
            schema_epoch,
            false,
            PreparedKind::Rollback,
        ));
    }

    // Only compute the lowercased SQL when the statement actually looks
    // like a PRAGMA. The pragma template needs case-folded matching on
    // many internal keywords; non-pragma statements should never pay
    // this allocation.
    if starts_with_pragma_keyword(stmt) {
        let lower = stmt.to_ascii_lowercase();
        // WS-A2f-rewrite: opt-in toggle for the DELETE/UPDATE ORDER BY/LIMIT
        // pre-parse rewrite. Intercepted here (before the pragma template
        // dispatcher rejects it as unknown) so the flag round-trips without
        // touching pragma.rs / session.rs. Returns a no-op template so the
        // SQL succeeds with zero side effects on the engine.
        if let Some(template) =
            try_parse_dml_order_limit_rewrite_pragma(trimmed, &lower, schema_epoch)
        {
            return Ok(template);
        }
        if let Some(template) = parse_pragma_template(conn, trimmed, &lower, schema_epoch, &schema)?
        {
            return Ok(template);
        }
    }

    if let Some(template) = parse_detach_template(trimmed, schema_epoch) {
        return Ok(template);
    }
    if let Some(template) = parse_attach_template(trimmed, schema_epoch) {
        return Ok(template);
    }
    if let Some(template) = templates::parse_reindex_template(trimmed, schema_epoch)? {
        return Ok(template);
    }
    if let Some(template) = templates::parse_vacuum_into_template(trimmed, schema_epoch)? {
        return Ok(template);
    }

    let dialect = SQLiteDialect {};
    let compat_sql = rewrite_sqlite_compat_syntax(sql);
    let sql_for_parser = prepare::strip_alter_add_column_if_not_exists_hint(
        &prepare::strip_cte_materialized_hints(&compat_sql),
    );
    // Phase 5 WS-A2e: clear any leftover table-index-hint state from a
    // previous (possibly errored) prepare before scanning the new SQL.
    prepare::reset_table_index_hints();
    let mut statements = match Parser::parse_sql(&dialect, &sql_for_parser) {
        Ok(statements) => statements,
        Err(first_err) => {
            // Track J: try the index-hint stripping rewrite first; if that
            // still fails, fall back to PostgreSqlDialect (which accepts
            // `RENAME CONSTRAINT` and a few other shapes the SQLite dialect
            // rejects). Both fallbacks preserve SELECT/DDL surfaces.
            let rewritten = match prepare::strip_sqlite_table_index_hints(&sql_for_parser) {
                Ok(rewritten) if rewritten != sql_for_parser => rewritten,
                _ => sql_for_parser.clone(),
            };
            match Parser::parse_sql(&dialect, &rewritten) {
                Ok(statements) => statements,
                Err(_) => {
                    let pg_dialect = sqlparser::dialect::PostgreSqlDialect {};
                    Parser::parse_sql(&pg_dialect, &sql_for_parser).map_err(|_| first_err)?
                }
            }
        }
    };
    prepare::apply_cte_materialized_hints(&mut statements, sql);
    if statements.len() != 1 {
        return Err(Error::UnsupportedSql(
            "only single-statement prepares are supported".to_owned(),
        ));
    }

    templates::bind_statement(conn, schema, schema_epoch, trimmed, statements.remove(0))
}

/// Allocation-free prefix check for the PRAGMA keyword. Mirrors
/// `parse_pragma_template`'s internal `lower.starts_with("pragma")`
/// gate so we can avoid lowercasing the full statement for the 99% of
/// non-pragma statements.
fn starts_with_pragma_keyword(stmt: &str) -> bool {
    let bytes = stmt.as_bytes();
    if bytes.len() < 6 {
        return false;
    }
    matches!(
        (
            bytes[0] | 0x20,
            bytes[1] | 0x20,
            bytes[2] | 0x20,
            bytes[3] | 0x20,
            bytes[4] | 0x20,
            bytes[5] | 0x20,
        ),
        (b'p', b'r', b'a', b'g', b'm', b'a')
    )
}

pub(crate) fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(msg) => *msg,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(msg) => (*msg).to_owned(),
            Err(_) => "non-string panic payload".to_owned(),
        },
    }
}

/// Detect a `DETACH [DATABASE] alias` statement before handing the SQL to
/// sqlparser (which does not recognise the SQLite DETACH form). Returns
/// `Some(template)` if the input matches the grammar, `None` otherwise.
pub(crate) fn parse_detach_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Option<PreparedTemplate> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let rest = strip_ignore_ascii_case_prefix(trimmed, b"detach database ")
        .or_else(|| strip_ignore_ascii_case_prefix(trimmed, b"detach "))?;
    let alias = rest.trim();
    if alias.is_empty() {
        return None;
    }
    Some(templates::template(
        trimmed,
        schema_epoch,
        false,
        PreparedKind::Attach(crate::exec::attach::AttachPlan::Detach {
            alias: Arc::from(alias),
        }),
    ))
}

pub(crate) fn parse_attach_template(
    sql: &str,
    schema_epoch: SchemaEpoch,
) -> Option<PreparedTemplate> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let rest = strip_ignore_ascii_case_prefix(trimmed, b"attach database ")
        .or_else(|| strip_ignore_ascii_case_prefix(trimmed, b"attach "))?;
    let (path_part, alias_part) = split_attach_path_alias(rest)?;
    let alias = alias_part.trim();
    if alias.is_empty() {
        return None;
    }
    Some(templates::template(
        trimmed,
        schema_epoch,
        false,
        PreparedKind::Attach(crate::exec::attach::AttachPlan::Attach {
            path: std::path::PathBuf::from(path_part),
            alias: Arc::from(alias),
        }),
    ))
}

fn split_attach_path_alias(rest: &str) -> Option<(String, &str)> {
    let rest = rest.trim_start();
    let bytes = rest.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let (path, after) = if bytes[0] == b'\'' || bytes[0] == b'"' {
        let quote = bytes[0];
        let mut i = 1usize;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == quote {
                return Some((out, parse_attach_alias(&rest[i + 1..])?));
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        return None;
    } else {
        let idx = rest.find(char::is_whitespace)?;
        (rest[..idx].to_owned(), &rest[idx..])
    };
    Some((path, parse_attach_alias(after)?))
}

fn parse_attach_alias(rest: &str) -> Option<&str> {
    let rest = rest.trim_start();
    strip_ignore_ascii_case_prefix(rest, b"as ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_exclude_current_row_with_existing_partition() {
        let sql = "SELECT sum(v) OVER (PARTITION BY g ORDER BY k ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE CURRENT ROW) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_current_row__', g"),
            "got: {out}"
        );
        assert!(!out.to_ascii_lowercase().contains("exclude"), "got: {out}");
    }

    #[test]
    fn rewrite_exclude_group_no_partition() {
        let sql = "SELECT count(*) OVER (ORDER BY k ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE GROUP) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_group__'"),
            "got: {out}"
        );
        assert!(!out.to_ascii_lowercase().contains("exclude"), "got: {out}");
    }

    #[test]
    fn rewrite_exclude_ties() {
        let sql = "SELECT first_value(v) OVER (PARTITION BY g ORDER BY k EXCLUDE TIES) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_ties__', g"),
            "got: {out}"
        );
    }

    #[test]
    fn rewrite_exclude_no_others() {
        let sql = "SELECT sum(v) OVER (PARTITION BY g ORDER BY k EXCLUDE NO OTHERS) FROM w";
        let out = rewrite_sqlite_compat_syntax(sql);
        assert!(
            out.contains("PARTITION BY '__redline_exc_no_others__', g"),
            "got: {out}"
        );
    }

    #[test]
    fn ascii_search_returns_match_offset() {
        assert_eq!(
            find_ignore_ascii_case("SELECT WINDOW win AS (x)", b" window win as ("),
            Some(6)
        );
        assert_eq!(
            find_ignore_ascii_case("SELECT 1", b" window win as ("),
            None
        );
    }

    #[test]
    fn attach_alias_prefix_is_case_insensitive() {
        assert_eq!(parse_attach_alias("  AS aux"), Some("aux"));
        assert_eq!(parse_attach_alias("  as aux"), Some("aux"));
        assert_eq!(parse_attach_alias("  aS aux"), Some("aux"));
    }
}
