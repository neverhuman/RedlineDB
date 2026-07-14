//! Round-trip smoke test against a running `redlinedb-server`.
//! Usage: redlinedb-client-smoke [ADDR]   (default 127.0.0.1:6033)

use redlinedb_client::{Client, Value};

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:6033".to_owned());
    match run(&addr) {
        Ok(()) => println!("SMOKE PASS ({addr})"),
        Err(e) => {
            eprintln!("SMOKE FAIL ({addr}): {e}");
            std::process::exit(1);
        }
    }
}

fn run(addr: &str) -> redlinedb_client::Result<()> {
    let mut c = Client::connect(addr)?;
    println!("connected + handshaked");

    // Fresh table (namespaced like a real consumer would prefix).
    c.execute("DROP TABLE IF EXISTS smoke_items")?;
    c.execute("CREATE TABLE smoke_items(id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL)")?;
    println!("created table");

    // Parameterized insert.
    c.execute_params(
        "INSERT INTO smoke_items(id, name, score) VALUES (?, ?, ?)",
        &[Value::Integer(1), Value::Text("ada".into()), Value::Real(9.5)],
    )?;
    // Non-parameterized insert.
    c.execute("INSERT INTO smoke_items(id, name, score) VALUES (2, 'lin', 8.0)")?;
    println!("inserted 2 rows");

    // Transaction with two more rows.
    c.transaction(|tx| {
        tx.execute_params(
            "INSERT INTO smoke_items(id, name, score) VALUES (?, ?, ?)",
            &[Value::Integer(3), Value::Text("grace".into()), Value::Real(10.0)],
        )?;
        tx.execute_params(
            "INSERT INTO smoke_items(id, name, score) VALUES (?, ?, ?)",
            &[Value::Integer(4), Value::Text("hopper".into()), Value::Real(9.9)],
        )?;
        Ok(())
    })?;
    println!("committed transaction (+2 rows)");

    // Count.
    let count = c.query_row("SELECT COUNT(*) FROM smoke_items", &[])?;
    let n = count[0].as_i64().unwrap_or(-1);
    println!("row count = {n}");
    assert_eq!(n, 4, "expected 4 rows");

    // Parameterized query.
    let r = c.query("SELECT id, name, score FROM smoke_items WHERE score >= ? ORDER BY id", &[Value::Real(9.0)])?;
    println!("query columns = {:?}", r.columns);
    for row in &r.rows {
        println!(
            "  id={} name={} score={:?}",
            row[0].as_i64().unwrap_or(-1),
            row[1].as_text().unwrap_or("?"),
            row[2]
        );
    }
    assert_eq!(r.rows.len(), 3, "expected 3 rows with score >= 9.0");

    c.close();
    Ok(())
}
