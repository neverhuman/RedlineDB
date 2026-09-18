//! SQL-02-R: correctness expectations; failures remain visible until fixed.
//! Expected values are separately qualified against the pinned CLI (see docs).
use redlinedb_sql::{Connection, Database, DbOptions, SqlValue, Step};

fn check(statements: &[&str], query: &str, expected: Vec<Vec<SqlValue>>) {
    let dir = tempfile::tempdir().unwrap();
    let database = Database::create(dir.path().join("db"), DbOptions::default()).unwrap();
    let connection = database.connect();
    for sql in statements {
        connection
            .execute(sql)
            .unwrap_or_else(|error| panic!("{sql}: {error:?}"));
    }
    assert_eq!(rows(&connection, query), expected, "{query}");
    drop(connection);
    drop(database);
    let reopened = Database::open(dir.path().join("db"), DbOptions::default()).unwrap();
    assert_eq!(
        rows(&reopened.connect(), query),
        expected,
        "after reopen: {query}"
    );
}

fn rows(connection: &std::sync::Arc<Connection>, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{sql}: {error:?}"));
    let mut rows = Vec::new();
    while let Step::Row = statement.step().unwrap() {
        rows.push(
            (0..statement.column_count())
                .map(|column| statement.column_value(column).unwrap().clone())
                .collect(),
        );
    }
    rows
}

#[test]
fn rename_table_preserves_unrelated_trigger_owner() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "CREATE TABLE b(x INTEGER)",
            "CREATE TABLE audit(x INTEGER)",
            "CREATE TRIGGER b_insert AFTER INSERT ON b BEGIN INSERT INTO audit VALUES(new.x); END",
            "ALTER TABLE a RENAME TO renamed_a",
            "INSERT INTO b VALUES(7)",
        ],
        "SELECT x FROM audit",
        vec![vec![SqlValue::Integer(7)]],
    );
}

#[test]
fn rename_column_preserves_unrelated_view_column() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "CREATE TABLE b(x INTEGER)",
            "INSERT INTO b VALUES(11)",
            "CREATE VIEW b_view AS SELECT x FROM b",
            "ALTER TABLE a RENAME COLUMN x TO y",
        ],
        "SELECT x FROM b_view",
        vec![vec![SqlValue::Integer(11)]],
    );
}

#[test]
fn rename_table_preserves_string_literal() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "INSERT INTO a VALUES(1)",
            "CREATE VIEW a_view AS SELECT 'a' AS label FROM a",
            "ALTER TABLE a RENAME TO renamed_a",
        ],
        "SELECT label FROM a_view",
        vec![vec![SqlValue::Text("a".into())]],
    );
}

#[test]
fn rename_table_updates_its_own_trigger_owner() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "CREATE TABLE audit(x INTEGER)",
            "CREATE TRIGGER a_insert AFTER INSERT ON a BEGIN INSERT INTO audit VALUES(new.x); END",
            "ALTER TABLE a RENAME TO renamed_a",
            "INSERT INTO renamed_a VALUES(7)",
        ],
        "SELECT x FROM audit",
        vec![vec![SqlValue::Integer(7)]],
    );
}

#[test]
fn rename_table_rollback_preserves_trigger_owner_and_literal() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "CREATE TABLE audit(x INTEGER)",
            "CREATE TRIGGER a_insert AFTER INSERT ON a BEGIN INSERT INTO audit VALUES(new.x); END",
            "CREATE VIEW a_view AS SELECT 'a' AS label FROM a",
            "BEGIN",
            "ALTER TABLE a RENAME TO renamed_a",
            "ROLLBACK",
            "INSERT INTO a VALUES(7)",
        ],
        "SELECT label, x FROM a_view CROSS JOIN audit",
        vec![vec![SqlValue::Text("a".into()), SqlValue::Integer(7)]],
    );
}

#[test]
fn rename_table_preserves_punctuation_in_quoted_identifier() {
    check(
        &[
            "CREATE TABLE a(\"q'r\" INTEGER, \"q--r\" INTEGER, \"q/*r\" INTEGER)",
            "INSERT INTO a VALUES(1, 2, 3)",
            "CREATE VIEW a_view AS SELECT \"q'r\", \"q--r\", \"q/*r\" FROM a",
            "ALTER TABLE a RENAME TO renamed_a",
        ],
        "SELECT * FROM a_view",
        vec![vec![
            SqlValue::Integer(1),
            SqlValue::Integer(2),
            SqlValue::Integer(3),
        ]],
    );
}

#[test]
fn rename_table_updates_quoted_table_reference() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "INSERT INTO a VALUES(1)",
            "CREATE VIEW a_view AS SELECT x FROM \"a\"",
            "ALTER TABLE a RENAME TO renamed_a",
        ],
        "SELECT * FROM a_view",
        vec![vec![SqlValue::Integer(1)]],
    );
}

#[test]
fn rename_table_escapes_new_name_in_quoted_references() {
    check(
        &[
            "CREATE TABLE a(x INTEGER)",
            "INSERT INTO a VALUES(1)",
            "CREATE VIEW a_view AS SELECT x FROM [a] UNION ALL SELECT x FROM \"a\"",
            "ALTER TABLE a RENAME TO \"a\"\"b]\"",
        ],
        "SELECT x FROM a_view ORDER BY x",
        vec![vec![SqlValue::Integer(1)], vec![SqlValue::Integer(1)]],
    );
}
