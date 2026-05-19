use redlinedb_sqlx::install_default_drivers;
use sqlx::{any::AnyPoolOptions, row::Row};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const HELPER_ROLE_ENV: &str = "REDLINEDB_SQLX_ATTACH_ROLE";
const HELPER_DB_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_DB_PATH";
const HELPER_READY_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_READY_PATH";
const HELPER_HOLD_PATH_ENV: &str = "REDLINEDB_SQLX_ATTACH_HOLD_PATH";

#[tokio::test]
async fn mode_rwc_still_opens_and_writes() {
    install_default_drivers();

    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("attach-mode-rwc.db");
    let url = format!("redline://{}?mode=rwc", db_path.display());

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect rwc");

    sqlx::query::query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create items");

    sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
        .bind(1_i64)
        .bind("Ada")
        .execute(&pool)
        .await
        .expect("insert item");

    let row = sqlx::query::query("SELECT name FROM items WHERE id = ?")
        .bind(1_i64)
        .fetch_one(&pool)
        .await
        .expect("select item");
    assert_eq!(row.try_get::<String, _>(0).unwrap(), "Ada");
}

#[tokio::test]
async fn mode_ro_attaches_to_live_database_and_blocks_writes() {
    install_default_drivers();

    let tempdir = tempfile::tempdir().expect("tempdir");
    let db_path = tempdir.path().join("attach-mode-ro.db");
    let ready_path = tempdir.path().join("owner.ready");
    let hold_path = tempdir.path().join("owner.hold");
    fs::write(&hold_path, b"hold").expect("create hold file");

    let mut child = spawn_helper(
        "owner_process_holds_database_open",
        &db_path,
        &ready_path,
        &hold_path,
    );

    wait_for_marker(
        &mut child,
        &ready_path,
        Instant::now() + Duration::from_secs(10),
    );

    let ro_url = format!("redline://{}?mode=ro", db_path.display());
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&ro_url)
        .await
        .expect("connect ro");

    let row = sqlx::query::query("SELECT name FROM items WHERE id = ?")
        .bind(1_i64)
        .fetch_one(&pool)
        .await
        .expect("read live row");
    assert_eq!(row.try_get::<String, _>(0).unwrap(), "Ada");

    let write_err = sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
        .bind(2_i64)
        .bind("Grace")
        .execute(&pool)
        .await
        .expect_err("read-only insert should fail");
    assert!(
        write_err.to_string().contains("read-only"),
        "unexpected read-only error: {write_err}"
    );

    drop(pool);

    let rwc_url = format!("redline://{}?mode=rwc", db_path.display());
    let owner_err = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&rwc_url)
        .await
        .expect_err("second writer should hit owner lock");
    assert!(
        owner_err.to_string().contains("database already open"),
        "unexpected owner-lock error: {owner_err}"
    );

    fs::remove_file(&hold_path).expect("release owner helper");
    reap_child(
        &mut child,
        Instant::now() + Duration::from_secs(10),
        "owner_process_holds_database_open",
    );
}

#[tokio::test]
async fn ro_attach_is_rejected_for_in_memory_urls() {
    install_default_drivers();

    let err = AnyPoolOptions::new()
        .max_connections(1)
        .connect("redline:///:memory:?mode=ro")
        .await
        .expect_err("read-only memory attach should fail");
    assert!(
        err.to_string()
            .contains("read-only attach mode is only supported for file-backed"),
        "unexpected memory attach error: {err}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn owner_process_holds_database_open() {
    match std::env::var(HELPER_ROLE_ENV) {
        Ok(role) if role == "owner" => {}
        _ => panic!("helper test must run with REDLINEDB_SQLX_ATTACH_ROLE=owner"),
    }

    install_default_drivers();

    let db_path = path_env(HELPER_DB_PATH_ENV);
    let ready_path = path_env(HELPER_READY_PATH_ENV);
    let hold_path = path_env(HELPER_HOLD_PATH_ENV);
    let url = format!("redline://{}?mode=rwc", db_path.display());

    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("owner connect");

    sqlx::query::query("CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await
        .expect("create items");

    sqlx::query::query("INSERT INTO items(id, name) VALUES (?, ?)")
        .bind(1_i64)
        .bind("Ada")
        .execute(&pool)
        .await
        .expect("seed live row");

    fs::write(&ready_path, b"ready").expect("write ready marker");

    let deadline = Instant::now() + Duration::from_secs(10);
    while hold_path.exists() {
        if Instant::now() > deadline {
            panic!("timed out waiting for parent to release hold file");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_helper(test_name: &str, db_path: &Path, ready_path: &Path, hold_path: &Path) -> Child {
    let current_exe = std::env::current_exe().expect("current exe");
    Command::new(current_exe)
        .args(["--exact", test_name, "--ignored", "--nocapture"])
        .env(HELPER_ROLE_ENV, "owner")
        .env(HELPER_DB_PATH_ENV, db_path)
        .env(HELPER_READY_PATH_ENV, ready_path)
        .env(HELPER_HOLD_PATH_ENV, hold_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn helper")
}

fn wait_for_marker(child: &mut Child, path: &Path, deadline: Instant) {
    loop {
        if path.exists() {
            return;
        }
        if let Some(status) = child.try_wait().expect("wait helper") {
            panic!("helper exited before writing marker: {status}");
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let status = child.wait().expect("wait helper after kill");
            panic!("timed out waiting for helper marker: {status}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn reap_child(child: &mut Child, deadline: Instant, label: &str) {
    loop {
        if let Some(status) = child.try_wait().expect("wait helper") {
            assert!(status.success(), "{label} failed: {status}");
            return;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let status = child.wait().expect("wait helper after kill");
            panic!("{label} did not exit before deadline: {status}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn path_env(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).expect("missing helper path env"))
}
