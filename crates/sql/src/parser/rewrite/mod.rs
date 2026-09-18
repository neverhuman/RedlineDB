//! Pre-parse SQL rewrites that lower SQLite/Postgres surface syntax into
//! shapes `sqlparser` and the executor already understand.

mod dml_limit;
mod pg_ddl;
mod pg_types;
mod scan;
mod sqlite_shape;
mod virtual_table;

pub(crate) use dml_limit::*;
pub(crate) use pg_ddl::*;
pub(crate) use pg_types::*;
pub(crate) use scan::*;
pub(crate) use sqlite_shape::*;
pub(crate) use virtual_table::*;

pub(crate) fn rewrite_sqlite_compat_syntax(sql: &str) -> String {
    let mut out = sql.to_owned();
    if contains_ignore_ascii_case(&out, b" window win as ")
        && let Some(spec) = extract_named_window_spec(&out, "win")
    {
        out = out.replace("OVER win", &format!("OVER ({spec})"));
        out = strip_window_clause(&out, "win");
    }
    if has_window_exclude(&out) {
        out = rewrite_window_exclude(&out);
    }
    if contains_on_conflict_clause(&out) {
        out = wrap_insert_select_with_upsert(&out);
        out = rewrite_on_conflict_clauses(&out);
    }
    out = rewrite_glob_to_function(&out);
    out = out.replace("NULL IS NOT 1", "NULL IS DISTINCT FROM 1");
    if has_jsonb_question_op(&out) {
        out = rewrite_jsonb_question_ops(&out);
    }
    if contains_ignore_ascii_case(&out, b"using ") {
        out = strip_create_index_using_clause(&out);
    }
    out = rewrite_strict_without_rowid_combo(&out);
    out = rewrite_create_sequence_options_order(&out);
    out = rewrite_alter_column_drop_identity(&out);
    out = rewrite_overriding_system_value(&out);
    if has_pg_array_literal(&out) {
        out = rewrite_pg_array_literal(&out);
    }
    if has_pg_bytea_literal(&out) {
        out = rewrite_pg_bytea_literal(&out);
    }
    if contains_ignore_ascii_case(&out, b"array_length(") {
        out = rewrite_array_length_function(&out);
    }
    if contains_ignore_ascii_case(&out, b"array_agg(") {
        out = rewrite_array_agg_function(&out);
    }
    if out.contains("&&") {
        out = rewrite_pg_array_overlap(&out);
    }
    if has_postfix_index(&out) {
        out = rewrite_postfix_index(&out);
    }
    if contains_ignore_ascii_case(&out, b"at time zone") {
        out = rewrite_at_time_zone(&out);
    }
    if contains_ignore_ascii_case(&out, b"interval ") {
        out = rewrite_pg_interval_literal(&out);
    }
    if out.contains("'+") || out.contains("'-") {
        out = rewrite_date_arith_with_modifier(&out);
    }
    if contains_ignore_ascii_case(&out, b" into ") {
        out = rewrite_select_into_to_ctas(&out);
    }
    if contains_ignore_ascii_case(&out, b" group by rollup ")
        || contains_ignore_ascii_case(&out, b" group by cube ")
    {
        out = rewrite_rollup_cube_to_grouping_sets(&out);
    }
    if contains_ignore_ascii_case(&out, b" group by grouping sets ") {
        out = rewrite_grouping_sets_to_union_all(&out);
    }
    if contains_ignore_ascii_case(&out, b" join lateral ")
        || contains_ignore_ascii_case(&out, b",lateral ")
    {
        out = rewrite_join_lateral_to_subquery(&out);
    }
    if dml_order_limit_rewrite_enabled()
        && contains_ignore_ascii_case(&out, b"limit ")
        && (contains_ignore_ascii_case(&out, b"delete ")
            || contains_ignore_ascii_case(&out, b"update "))
    {
        out = rewrite_dml_order_limit_to_subquery(&out);
    }
    out
}
