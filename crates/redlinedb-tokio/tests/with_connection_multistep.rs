//! Pool::with_connection runs a multi-step closure on one connection.

use redlinedb_tokio::{Pool, params};

#[tokio::test]
async fn multistep_state_is_visible_within_closure() {
    let pool = Pool::open_in_memory().await.expect("pool");
    pool.execute(
        "CREATE TABLE log(id INTEGER PRIMARY KEY, msg TEXT NOT NULL)",
        params![],
    )
    .await
    .expect("create");

    let row_count = pool
        .with_connection(|conn| {
            conn.execute("INSERT INTO log(id, msg) VALUES (1, 'a')", ())?;
            conn.execute("INSERT INTO log(id, msg) VALUES (2, 'b')", ())?;
            conn.execute("INSERT INTO log(id, msg) VALUES (3, 'c')", ())?;
            // Multi-step: read inside the same closure
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM log")?;
            let mut rows = 0_i64;
            if let redlinedb_tokio::Step::Row(row) = stmt.step()? {
                rows = row.get::<i64>(0)?;
            }
            Ok(rows)
        })
        .await
        .expect("closure ok");
    assert_eq!(row_count, 3);
}
