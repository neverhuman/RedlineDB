//! End-to-end async smoke test you can run with:
//!     cargo run --example async_round_trip -p redlinedb-tokio
//!
//! Opens an in-memory pool, creates a table, inserts a few rows, and prints
//! them back. Exits 0 on success.

use redlinedb_tokio::{Pool, params};

#[tokio::main]
async fn main() -> redlinedb_tokio::Result<()> {
    let pool = Pool::open_in_memory().await?;
    pool.execute(
        "CREATE TABLE messages(id INTEGER PRIMARY KEY, body TEXT NOT NULL)",
        params![],
    )
    .await?;

    for (id, body) in ["hello", "world", "from", "redlinedb-tokio"]
        .iter()
        .enumerate()
    {
        pool.execute(
            "INSERT INTO messages(id, body) VALUES (?, ?)",
            params![(id + 1) as i64, *body],
        )
        .await?;
    }

    let rows = pool
        .fetch_all("SELECT id, body FROM messages ORDER BY id", params![])
        .await?;
    for row in &rows {
        println!("  {} = {}", row.try_get_i64(0)?, row.try_get_text(1)?);
    }
    println!("ok: {} rows", rows.len());
    Ok(())
}
