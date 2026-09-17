//! Regression coverage for the first SQLite scale P0 parity failures.

#[path = "parity_oracle/harness.rs"]
mod harness;

use harness::assert_parity;

#[test]
fn table_constraint_check_between_accepts_valid_row() {
    assert_parity(
        "
        CREATE TABLE t(
          id INTEGER PRIMARY KEY,
          email TEXT UNIQUE NOT NULL,
          score INT CHECK(score BETWEEN 0 AND 100),
          active INT DEFAULT 1
        );
        INSERT INTO t(email, score) VALUES('a@example.test',10);
        SELECT id,email,score,active FROM t;
        ",
    );
}

#[test]
fn strict_integer_rejects_text_storage() {
    assert_parity(
        "
        CREATE TABLE t(a INTEGER) STRICT;
        INSERT INTO t VALUES('not-an-int');
        ",
    );
}

#[test]
fn insert_select_accepts_union_all_source() {
    assert_parity(
        "
        CREATE TABLE t(a INT, b TEXT);
        INSERT INTO t SELECT 1,'a' UNION ALL SELECT 2,'b';
        SELECT group_concat(a||b, ',') FROM t ORDER BY a;
        ",
    );
}

#[test]
fn delete_then_aggregate_scan_stays_in_rowid_order() {
    assert_parity(
        "
        CREATE TABLE t(id INT PRIMARY KEY, v TEXT);
        INSERT INTO t VALUES(1,'a'),(2,'b'),(3,'c');
        DELETE FROM t WHERE id=2;
        SELECT group_concat(v,'') FROM t ORDER BY id;
        ",
    );
}

#[test]
fn without_rowid_schema_persists_clause() {
    assert_parity(
        "
        CREATE TABLE t(a TEXT, b INT, PRIMARY KEY(a,b)) WITHOUT ROWID;
        INSERT INTO t VALUES('x',1);
        SELECT sql LIKE '%WITHOUT ROWID%' FROM sqlite_schema WHERE name='t';
        ",
    );
}

#[test]
fn strict_without_rowid_combo_accepts_sqlite_option_orders() {
    for create_sql in [
        "CREATE TABLE t(a INT, b TEXT, PRIMARY KEY(a,b)) STRICT, WITHOUT ROWID;",
        "CREATE TABLE t(a INT, b TEXT, PRIMARY KEY(a,b)) WITHOUT ROWID, STRICT;",
    ] {
        assert_parity(&format!(
            "
            {create_sql}
            INSERT INTO t VALUES(1,'a'),(2,'b');
            SELECT * FROM t ORDER BY a;
            SELECT typeof(a), typeof(b) FROM t LIMIT 1;
            "
        ));
    }
}

#[test]
fn without_rowid_rewrite_ignores_strict_inside_literals() {
    assert_parity(
        "
        CREATE TABLE t(a TEXT CHECK(a <> 'STRICT, WITHOUT ROWID'), PRIMARY KEY(a)) WITHOUT ROWID;
        SELECT strict FROM pragma_table_list WHERE name='t';
        ",
    );
}

#[test]
fn without_rowid_hides_implicit_rowid_aliases() {
    assert_parity(
        "
        CREATE TABLE t(a TEXT, b INT, PRIMARY KEY(a,b)) WITHOUT ROWID;
        INSERT INTO t VALUES('x',1);
        SELECT rowid FROM t;
        ",
    );
}
