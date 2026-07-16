//! db-shim parity self-test: the SAME code path must pass on both backends.
//! Run twice:  DB_BACKEND=sqlite DB_DSN=:memory: DB_NAMESPACE=demo db-shim-parity
//!             DB_BACKEND=redline DB_DSN=redline://127.0.0.1:6033 DB_NAMESPACE=demo db-shim-parity

use db_shim::{Db, Value};

fn main() {
    let backend = std::env::var("DB_BACKEND").unwrap_or_else(|_| "sqlite".into());
    let dsn = std::env::var("DB_DSN").unwrap_or_else(|_| ":memory:".into());
    let ns = std::env::var("DB_NAMESPACE").unwrap_or_else(|_| "demo".into());
    match run(&backend, &dsn, &ns) {
        Ok(()) => println!("DB-SHIM PARITY PASS (backend={backend} ns={ns})"),
        Err(e) => {
            eprintln!("DB-SHIM PARITY FAIL (backend={backend} ns={ns}): {e}");
            std::process::exit(1);
        }
    }
}

fn run(backend: &str, dsn: &str, ns: &str) -> db_shim::Result<()> {
    let mut db = Db::open(backend, dsn, ns)?;

    // Namespaced schema (the {ns} token is the only table-reference change a consumer makes).
    db.execute("DROP TABLE IF EXISTS {ns}items")?;
    db.execute("CREATE TABLE {ns}items(id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL)")?;

    // Parameterized + non-parameterized DML.
    db.execute_params(
        "INSERT INTO {ns}items(id,name,score) VALUES (?,?,?)",
        &[
            Value::Integer(1),
            Value::Text("ada".into()),
            Value::Real(9.5),
        ],
    )?;
    db.execute("INSERT INTO {ns}items(id,name,score) VALUES (2,'lin',8.0)")?;

    // Transaction.
    db.transaction(|tx| {
        tx.execute_params(
            "INSERT INTO {ns}items(id,name,score) VALUES (?,?,?)",
            &[
                Value::Integer(3),
                Value::Text("grace".into()),
                Value::Real(10.0),
            ],
        )?;
        Ok(())
    })?;

    // Pragma round-trip (parity feature both backends support).
    db.execute_batch("PRAGMA user_version = 7")?;
    let uv = db.query_row("PRAGMA user_version", &[])?;
    assert_eq!(
        uv.first(),
        Some(&Value::Integer(7)),
        "user_version round-trip"
    );

    // Count + parameterized query.
    let count = db.query_row("SELECT COUNT(*) FROM {ns}items", &[])?;
    assert_eq!(count.first(), Some(&Value::Integer(3)), "row count");
    let rows = db.query(
        "SELECT id, name FROM {ns}items WHERE score >= ? ORDER BY id",
        &[Value::Real(9.0)],
    )?;
    assert_eq!(rows.len(), 2, "expected 2 rows with score >= 9.0");
    println!("  rows(score>=9.0) = {rows:?}");
    Ok(())
}
