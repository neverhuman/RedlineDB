//! 16 concurrent tasks each insert 100 rows. Total = 1600. The shared
//! Database + serialized blocking tasks (via the semaphore) must produce a
//! consistent count — no corruption, no lost writes.

use redlinedb_tokio::{Pool, params};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixteen_writers_no_lost_rows() {
    let pool = Arc::new(Pool::open_in_memory().await.expect("pool"));
    pool.execute(
        "CREATE TABLE log(id INTEGER PRIMARY KEY, writer INTEGER NOT NULL, n INTEGER NOT NULL)",
        params![],
    )
    .await
    .expect("create");

    let mut handles = Vec::new();
    for writer in 0..16_i64 {
        let pool = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            for n in 0..100_i64 {
                // No AUTOINCREMENT — compose a unique id from (writer, n).
                let id = writer * 1000 + n;
                pool.execute(
                    "INSERT INTO log(id, writer, n) VALUES (?, ?, ?)",
                    params![id, writer, n],
                )
                .await
                .expect("insert");
            }
        }));
    }
    for h in handles {
        h.await.expect("task");
    }

    let row = pool
        .fetch_one("SELECT COUNT(*) FROM log", params![])
        .await
        .expect("count");
    assert_eq!(row.try_get_i64(0).unwrap(), 1600);

    let per_writer = pool
        .fetch_all(
            "SELECT writer, COUNT(*) FROM log GROUP BY writer ORDER BY writer",
            params![],
        )
        .await
        .expect("per-writer");
    assert_eq!(per_writer.len(), 16);
    for (i, row) in per_writer.iter().enumerate() {
        assert_eq!(row.try_get_i64(0).unwrap(), i as i64);
        assert_eq!(row.try_get_i64(1).unwrap(), 100);
    }
}
