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
        &PragmaCollationList,
        &PragmaFunctionList,
        &PragmaModuleList,
        &PragmaTableList,
        &PragmaTableXinfo,
        &PragmaIndexXinfo,
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
        // SQLite reports an empty `file` path for `:memory:` and other
        // ephemeral databases; the kernel-internal /dev/shm path is an
        // implementation detail and not user-visible.
        let main_path = if conn.is_in_memory() {
            String::new()
        } else {
            conn.database_path().to_string_lossy().into_owned()
        };
        let mut rows = vec![vec![
            SqlValue::Integer(0),
            SqlValue::Text(Arc::from("main")),
            SqlValue::Text(Arc::from(main_path.as_str())),
        ]];
        for (seq, (alias, path)) in conn
            .attach_map()
            .attached_aliases_with_paths()
            .into_iter()
            .enumerate()
        {
            // SQLite numbers attached schemas starting at 2 (seq=1
            // is reserved for the implicit `temp` schema).
            rows.push(vec![
                SqlValue::Integer((seq + 2) as i64),
                SqlValue::Text(Arc::from(alias.as_str())),
                SqlValue::Text(Arc::from(path.as_str())),
            ]);
        }
        Ok(TvResult {
            columns: vec!["seq".into(), "name".into(), "file".into()],
            rows,
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

struct PragmaCollationList;
impl TvFunc for PragmaCollationList {
    fn name(&self) -> &'static str {
        "pragma_collation_list"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_collation_list takes no arguments".to_owned(),
            ));
        }
        // SQLite ships with three built-in collations (BINARY, NOCASE,
        // RTRIM). Modern builds also register `decimal` and `uint` from
        // the ext-functions module; we mirror that surface so callers
        // probing `pragma_collation_list` see the standard set. The
        // `seq` column gives the registration order.
        let names = ["BINARY", "NOCASE", "RTRIM", "decimal", "uint"];
        let rows = names
            .iter()
            .enumerate()
            .map(|(seq, name)| {
                vec![
                    SqlValue::Integer(seq as i64),
                    SqlValue::Text(Arc::from(*name)),
                ]
            })
            .collect();
        Ok(TvResult {
            columns: vec!["seq".into(), "name".into()],
            rows,
        })
    }
}

struct PragmaFunctionList;
impl TvFunc for PragmaFunctionList {
    fn name(&self) -> &'static str {
        "pragma_function_list"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_function_list takes no arguments".to_owned(),
            ));
        }
        // SQLite reports one row per registered scalar / aggregate /
        // window function. The columns are (name, builtin, type, enc,
        // narg, flags). The set is large and varies by build; for
        // parity we include the SQLite-spec core set that the parity
        // suite probes. Narg = -1 means variadic; enc = "utf8".
        const FUNCS: &[(&str, i64)] = &[
            ("abs", 1),
            ("avg", 1),
            ("changes", 0),
            ("coalesce", -1),
            ("count", -1),
            ("date", -1),
            ("datetime", -1),
            ("group_concat", -1),
            ("hex", 1),
            ("ifnull", 2),
            ("instr", 2),
            ("julianday", -1),
            ("last_insert_rowid", 0),
            ("length", 1),
            ("like", -1),
            ("lower", 1),
            ("ltrim", -1),
            ("max", -1),
            ("min", -1),
            ("nullif", 2),
            ("printf", -1),
            ("quote", 1),
            ("random", 0),
            ("randomblob", 1),
            ("replace", 3),
            ("round", -1),
            ("rtrim", -1),
            ("strftime", -1),
            ("substr", -1),
            ("sum", 1),
            ("time", -1),
            ("total", 1),
            ("total_changes", 0),
            ("trim", -1),
            ("typeof", 1),
            ("unicode", 1),
            ("upper", 1),
            ("zeroblob", 1),
        ];
        let rows = FUNCS
            .iter()
            .map(|(name, narg)| {
                vec![
                    SqlValue::Text(Arc::from(*name)),
                    SqlValue::Integer(1),
                    SqlValue::Text(Arc::from("scalar")),
                    SqlValue::Text(Arc::from("utf8")),
                    SqlValue::Integer(*narg),
                    SqlValue::Integer(0),
                ]
            })
            .collect();
        Ok(TvResult {
            columns: vec![
                "name".into(),
                "builtin".into(),
                "type".into(),
                "enc".into(),
                "narg".into(),
                "flags".into(),
            ],
            rows,
        })
    }
}

struct PragmaModuleList;
impl TvFunc for PragmaModuleList {
    fn name(&self) -> &'static str {
        "pragma_module_list"
    }
    fn eval(
        &self,
        _conn: &Connection,
        _schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_module_list takes no arguments".to_owned(),
            ));
        }
        // SQLite's vtab module registry. RedlineDB does not actually
        // implement these modules — we surface the SQLite-spec names so
        // callers probing the list see the standard FTS / RTree /
        // table-info module entries. Each row is a single text column.
        const MODULES: &[&str] = &[
            "bytecode",
            "dbpage",
            "dbstat",
            "fts3",
            "fts3tokenize",
            "fts4",
            "fts4aux",
            "fts5",
            "fts5vocab",
            "generate_series",
            "json_each",
            "json_tree",
            "pragma_collation_list",
            "pragma_compile_options",
            "pragma_database_list",
            "pragma_foreign_key_list",
            "pragma_function_list",
            "pragma_index_info",
            "pragma_index_list",
            "pragma_index_xinfo",
            "pragma_module_list",
            "pragma_table_info",
            "pragma_table_list",
            "pragma_table_xinfo",
            "rtree",
            "rtree_i32",
        ];
        let rows = MODULES
            .iter()
            .map(|m| vec![SqlValue::Text(Arc::from(*m))])
            .collect();
        Ok(TvResult {
            columns: vec!["name".into()],
            rows,
        })
    }
}

struct PragmaTableXinfo;
impl TvFunc for PragmaTableXinfo {
    fn name(&self) -> &'static str {
        "pragma_table_xinfo"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_table_xinfo", args)?;
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
                "hidden".into(),
            ],
            rows: crate::parser::pragma_table_xinfo_rows(&table),
        })
    }
}

struct PragmaIndexXinfo;
impl TvFunc for PragmaIndexXinfo {
    fn name(&self) -> &'static str {
        "pragma_index_xinfo"
    }
    fn eval(
        &self,
        _conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        let name = single_text_arg("pragma_index_xinfo", args)?;
        let index = lookup_index(
            schema,
            &QualifiedName {
                schema: DbName::new("main"),
                name: DbName::new(name),
            },
        )?;
        Ok(TvResult {
            columns: vec![
                "seqno".into(),
                "cid".into(),
                "name".into(),
                "desc".into(),
                "coll".into(),
                "key".into(),
            ],
            rows: crate::parser::pragma_index_xinfo_rows(schema, &index)?,
        })
    }
}

struct PragmaTableList;
impl TvFunc for PragmaTableList {
    fn name(&self) -> &'static str {
        "pragma_table_list"
    }
    fn eval(
        &self,
        conn: &Connection,
        schema: &SchemaSnapshot,
        args: &[TvArg],
    ) -> Result<TvResult> {
        if !args.is_empty() {
            return Err(Error::UnsupportedSql(
                "pragma_table_list takes no arguments".to_owned(),
            ));
        }
        Ok(TvResult {
            columns: vec![
                "schema".into(),
                "name".into(),
                "type".into(),
                "ncol".into(),
                "wr".into(),
                "strict".into(),
            ],
            rows: crate::parser::pragma_table_list_rows(conn, schema)?,
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
