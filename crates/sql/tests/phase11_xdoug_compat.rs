mod common;

use common::open_database;
use redlinedb_sql::Step;

#[test]
fn update_subquery_with_order_by_limit_works() {
    let (_dir, conn) = open_database();

    conn.execute("CREATE TABLE src(kind TEXT, label TEXT, rank INTEGER)")
        .expect("create src");
    conn.execute("CREATE TABLE dst(id INTEGER PRIMARY KEY, kind TEXT, label TEXT)")
        .expect("create dst");
    conn.execute(
        "INSERT INTO src(kind, label, rank) VALUES
         ('alpha', 'low', 1),
         ('alpha', 'high', 9),
         ('beta', 'mid', 5)",
    )
    .expect("seed src");
    conn.execute(
        "INSERT INTO dst(id, kind, label) VALUES
         (1, 'alpha', 'old'),
         (2, 'beta', 'old')",
    )
    .expect("seed dst");

    conn.execute(
        "UPDATE dst
         SET label = (
             SELECT label
             FROM src
             ORDER BY rank DESC, label ASC
             LIMIT 1
         )
         WHERE id = 1",
    )
    .expect("update subquery");

    let mut stmt = conn
        .prepare("SELECT id, label FROM dst ORDER BY id")
        .expect("select dst");
    assert_eq!(stmt.step().expect("row 1"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 1);
    assert_eq!(stmt.column_text(1).expect("label 1"), "high");
    assert_eq!(stmt.step().expect("row 2"), Step::Row);
    assert_eq!(stmt.column_i64(0).expect("id"), 2);
    assert_eq!(stmt.column_text(1).expect("label 2"), "old");
    assert_eq!(stmt.step().expect("done"), Step::Done);
}
