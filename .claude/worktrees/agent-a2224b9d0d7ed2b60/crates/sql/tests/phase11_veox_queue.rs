mod common;

use std::sync::{Arc, Barrier, Mutex};
use std::thread;

use common::step_done;
use redlinedb_sql::{BeginMode, Database, DbOptions, Error, Step};
use tempfile::tempdir;

fn open_queue_db() -> (tempfile::TempDir, Arc<redlinedb_sql::Database>) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("queue.db");
    let db = Database::create(&path, DbOptions::default()).expect("create db");
    (dir, db)
}

fn seed_queue(db: &Arc<redlinedb_sql::Database>, rows: &[(i64, i64, i64)]) {
    let conn = db.connect();
    conn.execute(
        "CREATE TABLE queue_jobs(
            id INTEGER PRIMARY KEY,
            state INTEGER NOT NULL,
            priority INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            attempts INTEGER NOT NULL
        )",
    )
    .expect("create table");
    conn.execute("CREATE INDEX queue_state_idx ON queue_jobs(state)")
        .expect("create index");
    for &(id, priority, created_at) in rows {
        let mut stmt = conn
            .prepare(
                "INSERT INTO queue_jobs(id, state, priority, created_at, attempts) VALUES (?1, 0, ?2, ?3, 0)",
            )
            .expect("prepare insert");
        stmt.bind_i64(1, id).expect("bind id");
        stmt.bind_i64(2, priority).expect("bind priority");
        stmt.bind_i64(3, created_at).expect("bind created_at");
        step_done(&mut stmt);
    }
}

fn is_retryable_claim_error(err: &Error) -> bool {
    matches!(err, Error::Kernel(_)) && format!("{err:?}").contains("SerializationFailure")
}

fn claim_one(conn: &Arc<redlinedb_sql::Connection>) -> Option<(i64, i64, i64)> {
    loop {
        match claim_one_once(conn) {
            Ok(Some(row)) => return Some(row),
            Ok(None) => return None,
            Err(err) if is_retryable_claim_error(&err) => {
                let _ = conn.rollback();
                thread::yield_now();
                continue;
            }
            Err(err) => panic!("claim failed: {err:?}"),
        }
    }
}

fn claim_one_once(conn: &Arc<redlinedb_sql::Connection>) -> Result<Option<(i64, i64, i64)>, Error> {
    conn.begin(BeginMode::Immediate)?;
    let outcome = (|| -> Result<Option<(i64, i64, i64)>, Error> {
        let mut next = conn.prepare(
            "SELECT id, priority, created_at
             FROM queue_jobs
             WHERE state = 0
             ORDER BY priority DESC, created_at ASC, id ASC
             LIMIT 1",
        )?;
        let Step::Row = next.step()? else {
            return Ok(None);
        };
        let id = next.column_i64(0)?;
        let priority = next.column_i64(1)?;
        let created_at = next.column_i64(2)?;
        step_done(&mut next);

        let mut claim = conn.prepare(
            "UPDATE queue_jobs
             SET state = 1, attempts = attempts + 1
             WHERE id = ?1 AND state = 0
             RETURNING id",
        )?;
        claim.bind_i64(1, id)?;
        assert_eq!(claim.step()?, Step::Row);
        assert_eq!(claim.column_i64(0)?, id);
        step_done(&mut claim);
        Ok(Some((id, priority, created_at)))
    })();
    match outcome {
        Ok(value) => {
            conn.commit()?;
            Ok(value)
        }
        Err(err) => {
            let _ = conn.rollback();
            Err(err)
        }
    }
}

#[test]
fn queue_claims_follow_priority_created_at_and_id_order() {
    let (_dir, db) = open_queue_db();
    seed_queue(
        &db,
        &[
            (10, 1, 30),
            (11, 3, 50),
            (12, 3, 40),
            (13, 3, 40),
            (14, 2, 10),
        ],
    );
    let conn = db.connect();

    let mut claimed = Vec::new();
    while let Some((id, priority, created_at)) = claim_one(&conn) {
        claimed.push((id, priority, created_at));
    }

    assert_eq!(
        claimed,
        vec![
            (12, 3, 40),
            (13, 3, 40),
            (11, 3, 50),
            (14, 2, 10),
            (10, 1, 30),
        ]
    );
}

#[test]
fn queue_claims_are_unique_under_contention() {
    let (_dir, db) = open_queue_db();
    seed_queue(
        &db,
        &[
            (1, 9, 10),
            (2, 8, 11),
            (3, 7, 12),
            (4, 6, 13),
            (5, 5, 14),
            (6, 4, 15),
            (7, 3, 16),
            (8, 2, 17),
        ],
    );
    let conn1 = db.connect();
    let conn2 = db.connect();
    let barrier = Arc::new(Barrier::new(3));
    let claimed = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::new();
    for conn in [conn1, conn2] {
        let barrier = Arc::clone(&barrier);
        let claimed = Arc::clone(&claimed);
        handles.push(thread::spawn(move || {
            barrier.wait();
            while let Some((id, _, _)) = claim_one(&conn) {
                claimed.lock().expect("claimed").push(id);
            }
        }));
    }

    barrier.wait();
    for handle in handles {
        handle.join().expect("worker");
    }

    let mut claimed = claimed.lock().expect("claimed").clone();
    claimed.sort();
    claimed.dedup();
    assert_eq!(claimed, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn queue_complete_and_fail_require_a_claimed_row() {
    let (_dir, db) = open_queue_db();
    seed_queue(&db, &[(1, 9, 10), (2, 1, 20)]);
    let conn = db.connect();

    let mut complete = conn
        .prepare("UPDATE queue_jobs SET state = 2 WHERE id = ?1 AND state = 1 RETURNING id")
        .expect("prepare complete");
    complete.bind_i64(1, 1).expect("bind pending id");
    assert_eq!(complete.step().expect("step"), Step::Done);

    assert_eq!(claim_one(&conn), Some((1, 9, 10)));

    let mut complete = conn
        .prepare("UPDATE queue_jobs SET state = 2 WHERE id = ?1 AND state = 1 RETURNING id")
        .expect("prepare complete");
    complete.bind_i64(1, 1).expect("bind claimed id");
    assert_eq!(complete.step().expect("step"), Step::Row);
    assert_eq!(complete.column_i64(0).expect("id"), 1);
    step_done(&mut complete);

    let mut fail = conn
        .prepare("UPDATE queue_jobs SET state = 3 WHERE id = ?1 AND state = 1 RETURNING id")
        .expect("prepare fail");
    fail.bind_i64(1, 1).expect("bind completed id");
    assert_eq!(fail.step().expect("step"), Step::Done);

    let mut state = conn
        .prepare("SELECT state, attempts FROM queue_jobs WHERE id = 1")
        .expect("prepare state");
    assert_eq!(state.step().expect("step"), Step::Row);
    assert_eq!(state.column_i64(0).expect("state"), 2);
    assert_eq!(state.column_i64(1).expect("attempts"), 1);
    step_done(&mut state);
}
