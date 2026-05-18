//! Table-valued PRAGMA wrappers.
//!
//! Provides `pragma_table_info`, `pragma_index_list`, `pragma_index_info`,
//! `pragma_foreign_key_list`, and `pragma_database_list` as table-valued
//! functions usable in `FROM` / `JOIN` clauses. The metadata source is
//! shared with the bare `PRAGMA name(arg)` forms parsed in
//! `crate::parser::pragma`, so both surfaces return identical row sets.

use std::sync::Arc;

use redlinedb_kernel::catalog::{
    DbName, QualifiedName, SchemaSnapshot, lookup_index, lookup_table,
};

use crate::connection::Connection;
use crate::error::{Error, Result};
use crate::value::SqlValue;

use super::table_valued::{TvArg, TvFunc, TvResult};

pub(super) fn registry() -> &'static [&'static dyn TvFunc] {
    &[
        &PragmaTableInfo,
        &PragmaIndexList,
        &PragmaIndexInfo,
        &PragmaForeignKeyList,
        &PragmaDatabaseList,
        &PragmaCompileOptions,
    ]
}

struct PragmaTableInfo;
impl TvFunc for PragmaTableInfo {
    fn name(&self) -> &'static str {
        "pragma_table_info"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_table_info", args)?;
        let table = lookup_table(
            schema,
            &QualifiedName {
                schema: DbName::new("main"),
                name: DbName::new(name),
            },
        )?;
        Ok(TvResult {
            columns: vec![
                "cid".into(),
                "name".into(),
                "type".into(),
                "notnull".into(),
                "dflt_value".into(),
                "pk".into(),
            ],
            rows: crate::parser::pragma_table_info_rows(&table),
        })
    }
}

struct PragmaIndexList;
impl TvFunc for PragmaIndexList {
    fn name(&self) -> &'static str {
        "pragma_index_list"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_index_list", args)?;
        let table = lookup_table(
            schema,
            &QualifiedName {
                schema: DbName::new("main"),
                name: DbName::new(name),
            },
        )?;
        Ok(TvResult {
            columns: vec![
                "seq".into(),
                "name".into(),
                "unique".into(),
                "origin".into(),
                "partial".into(),
            ],
            rows: crate::parser::pragma_index_list_rows(&table),
        })
    }
}

struct PragmaIndexInfo;
impl TvFunc for PragmaIndexInfo {
    fn name(&self) -> &'static str {
        "pragma_index_info"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_index_info", args)?;
        let index = lookup_index(
            schema,
            &QualifiedName {
                schema: DbName::new("main"),
                name: DbName::new(name),
            },
        )?;
        Ok(TvResult {
            columns: vec!["seqno".into(), "cid".into(), "name".into()],
            rows: crate::parser::pragma_index_info_rows(schema, &index)?,
        })
    }
}

struct PragmaForeignKeyList;
impl TvFunc for PragmaForeignKeyList {
    fn name(&self) -> &'static str {
        "pragma_foreign_key_list"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_foreign_key_list", args)?;
        let table = lookup_table(
            schema,
            &QualifiedName {
                schema: DbName::new("main"),
                name: DbName::new(name),
            },
        )?;
        Ok(TvResult {
            columns: vec![
                "id".into(),
                "seq".into(),
                "table".into(),
                "from".into(),
                "to".into(),
                "on_update".into(),
                "on_delete".into(),
                "match".into(),
                "deferred".into(),
            ],
            rows: crate::parser::pragma_foreign_key_list_rows(&table),
        })
    }
}

struct PragmaDatabaseList;
impl TvFunc for PragmaDatabaseList {
    fn name(&self) -> &'static str {
        "pragma_database_list"
    }
    fn eval(
        &self,
        conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_database_list takes no arguments".to_owned(),
            ));
        }
        let row = vec![
            SqlValue::Integer(0),
            SqlValue::Text(Arc::from("main")),
            SqlValue::Text(Arc::from(conn.database_path().to_string_lossy().as_ref())),
        ];
        Ok(TvResult {
            columns: vec!["seq".into(), "name".into(), "file".into()],
            rows: vec![row],
        })
    }
}

struct PragmaCompileOptions;
impl TvFunc for PragmaCompileOptions {
    fn name(&self) -> &'static str {
        "pragma_compile_options"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_compile_options takes no arguments".to_owned(),
            ));
        }
        Ok(TvResult {
            columns: vec!["compile_options".into()],
            rows: crate::parser::pragma_compile_options_rows(),
        })
    }
}

fn single_text_arg(name: &str, args: &[TvArg]) -> Result<String> {
    match args {
        [arg] => match arg.as_text() {
            Some(text) => Ok(text.to_owned()),
            None => Err(Error::UnsupportedSql(format!(
                "{name} expects a single text-typed argument"
            ))),
        },
        other => Err(Error::UnsupportedSql(format!(
            "{name} expects exactly one argument (got {})",
            other.len()
        ))),
    }
}
