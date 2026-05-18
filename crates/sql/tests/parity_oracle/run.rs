//! Per-engine runners: take a SQL script, return an [`OracleResult`].

#![allow(dead_code)]

use std::sync::Arc;

use redlinedb_sql::{Connection, Database, DbOptions, Step};
use rusqlite::types::Value as RuValue;

use super::normalize::{maybe_sort, split_script, to_sql_value};
use super::types::{ErrorClass, OracleResult, classify_err};

fn open_redline() -> (tempfile::TempDir, Arc<Connection>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("parity_oracle.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db.connect())
}

/// Run a SQL script against rusqlite (the oracle).
pub fn run_oracle(sql: &str) -> OracleResult {
    let (setup, query) = split_script(sql);
    let conn = match rusqlite::Connection::open_in_memory() {
        Ok(c) => c,
        Err(e) => return OracleResult::err(classify_err(&e.to_string()), e.to_string()),
    };
    if !setup.is_empty()
        && let Err(e) = conn.execute_batch(&setup)
    {
        return OracleResult::err(classify_err(&e.to_string()), e.to_string());
    }
    let mut stmt = match conn.prepare(&query) {
        Ok(s) => s,
        Err(e) => return OracleResult::err(classify_err(&e.to_string()), e.to_string()),
    };
    let ncols = stmt.column_count();
    let mut rows = Vec::new();
    let mut q = match stmt.query([]) {
        Ok(q) => q,
        Err(e) => return OracleResult::err(classify_err(&e.to_string()), e.to_string()),
    };
    loop {
        match q.next() {
            Ok(Some(row)) => {
                let mut current = Vec::with_capacity(ncols);
                for i in 0..ncols {
                    let v: RuValue = match row.get(i) {
                        Ok(v) => v,
                        Err(e) => {
                            return OracleResult::err(classify_err(&e.to_string()), e.to_string());
                        }
                    };
                    current.push(to_sql_value(v));
                }
                rows.push(current);
            }
            Ok(None) => break,
            Err(e) => {
                return OracleResult::err(classify_err(&e.to_string()), e.to_string());
            }
        }
    }
    let mut rows = rows;
    maybe_sort(&query, &mut rows);
    OracleResult::ok(rows)
}

/// Run a SQL script against redlinedb.
pub fn run_redline(sql: &str) -> OracleResult {
    let (setup, query) = split_script(sql);
    let (_dir, conn) = open_redline();
    if !setup.is_empty()
        && let Err(e) = conn.execute(&setup)
    {
        let msg = format!("{e:?}");
        return OracleResult::err(classify_err(&msg), msg);
    }
    let mut stmt = match conn.prepare(&query) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("{e:?}");
            return OracleResult::err(classify_err(&msg), msg);
        }
    };
    let ncols = stmt.column_count();
    let mut rows = Vec::new();
    loop {
        match stmt.step() {
            Ok(Step::Row) => {
                let mut current = Vec::with_capacity(ncols);
                for i in 0..ncols {
                    current.push(stmt.column_value(i).expect("col").clone());
                }
                rows.push(current);
            }
            Ok(Step::Done) => break,
            Err(e) => {
                let msg = format!("{e:?}");
                return OracleResult::err(classify_err(&msg), msg);
            }
        }
    }
    maybe_sort(&query, &mut rows);
    OracleResult::ok(rows)
}

/// Public glue type so callers don't have to match on `Option<ErrorClass>`.
#[allow(unused)]
pub fn _suppress_unused(_e: ErrorClass) {}
