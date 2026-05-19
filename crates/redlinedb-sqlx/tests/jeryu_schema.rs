use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPoolOptions, row::Row};
use tempfile::tempdir;

#[tokio::test]
async fn any_pool_runs_jeryu_shaped_append_only_schema() {
    install_default_drivers();

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("redline:///:memory:")
        .await
        .expect("connect");

    sqlx::query::query(
        "CREATE TABLE events(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            state TEXT NOT NULL CHECK(state IN ('armed', 'paused')),
            payload TEXT NOT NULL DEFAULT '{}'
        )",
    )
    .execute(&pool)
    .await
    .expect("create events");
    sqlx::query::query(
        "CREATE TRIGGER events_no_update
         BEFORE UPDATE ON events
         FOR EACH ROW
         BEGIN
             SELECT RAISE(ABORT, 'events are append-only');
         END",
    )
    .execute(&pool)
    .await
    .expect("create update trigger");
    sqlx::query::query(
        "CREATE TRIGGER events_no_delete
         BEFORE DELETE ON events
         FOR EACH ROW
         BEGIN
             SELECT RAISE(ABORT, 'events are append-only');
         END",
    )
    .execute(&pool)
    .await
    .expect("create delete trigger");

    let inserted = sqlx::query::query("INSERT INTO events(state) VALUES ('armed')")
        .execute(&pool)
        .await
        .expect("insert event");
    let id = inserted
        .last_insert_id()
        .expect("redline bridge should report inserted rowid");
    assert!(id > 0);

    let row =
        sqlx::query::query("SELECT id, state, payload, last_insert_rowid() AS last_id FROM events")
            .fetch_one(&pool)
            .await
            .expect("select event");
    assert_eq!(row.try_get::<i64, _>("id").unwrap(), id);
    assert_eq!(row.try_get::<String, _>("state").unwrap(), "armed");
    assert_eq!(row.try_get::<String, _>("payload").unwrap(), "{}");
    assert_eq!(row.try_get::<i64, _>("last_id").unwrap(), id);

    let invalid = sqlx::query::query("INSERT INTO events(state) VALUES ('done')")
        .execute(&pool)
        .await
        .expect_err("invalid CHECK value should fail");
    assert!(
        invalid.to_string().contains("CHECK constraint failed"),
        "unexpected CHECK error: {invalid}"
    );

    let update = sqlx::query::query("UPDATE events SET state = 'paused' WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .expect_err("append-only update should fail");
    assert!(
        update.to_string().contains("events are append-only"),
        "unexpected update error: {update}"
    );

    let delete = sqlx::query::query("DELETE FROM events WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .expect_err("append-only delete should fail");
    assert!(
        delete.to_string().contains("events are append-only"),
        "unexpected delete error: {delete}"
    );

    let row = sqlx::query::query("SELECT count(*) AS n FROM events")
        .fetch_one(&pool)
        .await
        .expect("select surviving event count");
    assert_eq!(row.try_get::<i64, _>("n").unwrap(), 1);
    let row = sqlx::query::query("SELECT state FROM events WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .expect("select surviving event state");
    assert_eq!(row.try_get::<String, _>("state").unwrap(), "armed");
}

#[tokio::test]
async fn any_pool_file_schema_sees_ddl_across_connections() {
    install_default_drivers();

    let dir = tempdir().expect("temp dir");
    let url = format!("redline:{}?mode=rwc", dir.path().join("jeryu.db").display());
    let pool = AnyPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect");

    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS pools (
            name TEXT PRIMARY KEY,
            gitlab_runner_id INTEGER NOT NULL,
            auth_token TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '',
            executor TEXT NOT NULL DEFAULT 'docker',
            min_warm INTEGER NOT NULL DEFAULT 1,
            max_managers INTEGER NOT NULL DEFAULT 4,
            concurrent INTEGER NOT NULL DEFAULT 8,
            request_concurrency INTEGER NOT NULL DEFAULT 4,
            paused INTEGER NOT NULL DEFAULT 0,
            trust_tier TEXT NOT NULL DEFAULT 'trusted'
        )",
    )
    .execute(&pool)
    .await
    .expect("create pools");

    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS managers (
            id TEXT PRIMARY KEY,
            pool_name TEXT NOT NULL REFERENCES pools(name),
            docker_container_id TEXT NOT NULL UNIQUE,
            system_id TEXT,
            state TEXT NOT NULL DEFAULT 'starting',
            config_dir TEXT NOT NULL,
            started_at TEXT,
            last_contact_at TEXT
        )",
    )
    .execute(&pool)
    .await
    .expect("create managers");

    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS job_events (
            job_id INTEGER NOT NULL,
            project_id INTEGER NOT NULL,
            pipeline_id INTEGER,
            status TEXT NOT NULL,
            job_name TEXT,
            pool_name TEXT,
            system_id TEXT,
            queued_duration REAL,
            received_at TEXT NOT NULL,
            PRIMARY KEY (job_id, status)
        )",
    )
    .execute(&pool)
    .await
    .expect("create job_events");

    sqlx::query::query(
        "CREATE TABLE IF NOT EXISTS ci_job_runs (
            job_id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL,
            pipeline_id INTEGER NOT NULL,
            root_pipeline_id INTEGER NOT NULL,
            pipeline_sha TEXT NOT NULL,
            ref_name TEXT NOT NULL,
            job_name TEXT NOT NULL,
            stage TEXT NOT NULL,
            status TEXT NOT NULL,
            runner TEXT,
            runner_pool TEXT,
            queued_duration_secs REAL,
            duration_secs REAL,
            started_at TEXT,
            finished_at TEXT,
            web_url TEXT,
            observed_at TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .expect("create ci_job_runs");

    let row = sqlx::query::query("SELECT count(*) AS n FROM ci_job_runs")
        .fetch_one(&pool)
        .await
        .expect("read ci_job_runs before index");
    assert_eq!(row.try_get::<i64, _>("n").unwrap(), 0);

    sqlx::query::query(
        "CREATE INDEX IF NOT EXISTS idx_ci_job_runs_pipeline
         ON ci_job_runs(project_id, pipeline_id)",
    )
    .execute(&pool)
    .await
    .expect("create pooled index");

    sqlx::query::query(
        "CREATE INDEX IF NOT EXISTS idx_ci_job_runs_pipeline
         ON ci_job_runs(project_id, pipeline_id)",
    )
    .execute(&pool)
    .await
    .expect("duplicate pooled index should be ignored");
}
