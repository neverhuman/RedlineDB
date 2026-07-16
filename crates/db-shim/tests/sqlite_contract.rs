use db_shim::{Db, Error};
#[cfg(feature = "sqlite-parity")]
use db_shim::{Result, Value};
#[cfg(feature = "sqlite-parity")]
use proptest::prelude::*;

#[test]
fn release_identity_is_native_redline_4_1_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "4.1.0");
}

#[test]
#[cfg(feature = "sqlite-parity")]
fn sqlite_round_trip_expands_namespace() {
    let mut db = Db::open("sqlite", ":memory:", "orders").expect("open sqlite");
    db.execute("CREATE TABLE {ns}items(id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .expect("create namespaced table");
    db.execute_params(
        "INSERT INTO {ns}items(id, name) VALUES (?, ?)",
        &[Value::Integer(1), Value::Text("ada".to_owned())],
    )
    .expect("insert row");

    let rows = db
        .query("SELECT id, name FROM {ns}items", &[])
        .expect("query row");
    assert_eq!(
        rows,
        vec![vec![Value::Integer(1), Value::Text("ada".to_owned())]]
    );
    let table = db
        .query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='orders_items'",
            &[],
        )
        .expect("namespaced table exists");
    assert_eq!(table, vec![Value::Text("orders_items".to_owned())]);
}

#[test]
#[cfg(feature = "sqlite-parity")]
fn failed_transaction_rolls_back() {
    let mut db = Db::open("sqlite", ":memory:", "tx").expect("open sqlite");
    db.execute("CREATE TABLE {ns}items(id INTEGER PRIMARY KEY)")
        .expect("create table");
    let result: Result<()> = db.transaction(|tx| {
        tx.execute("INSERT INTO {ns}items(id) VALUES (1)")?;
        Err(Error::Config("force rollback".to_owned()))
    });
    assert!(matches!(result, Err(Error::Config(message)) if message == "force rollback"));
    let count = db
        .query_row("SELECT COUNT(*) FROM {ns}items", &[])
        .expect("count rows");
    assert_eq!(count, vec![Value::Integer(0)]);
}

#[test]
fn unknown_backend_fails_closed() {
    let error = match Db::open("memory", ":memory:", "") {
        Ok(_) => panic!("unknown backend must fail"),
        Err(error) => error,
    };
    let expected = if cfg!(feature = "sqlite-parity") {
        "redline|sqlite"
    } else {
        "redline"
    };
    assert!(matches!(error, Error::Config(message) if message.contains(expected)));
}

#[cfg(not(feature = "sqlite-parity"))]
#[test]
fn default_build_rejects_sqlite_backend() {
    let error = match Db::open("sqlite", ":memory:", "") {
        Ok(_) => panic!("default build must not contain the SQLite backend"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        Error::Config(message)
            if message.contains("requires the sqlite-parity feature")
                && message.contains("Redline-only")
    ));
}

#[cfg(feature = "sqlite-parity")]
proptest! {
    #[test]
    fn valid_namespaces_are_isolated(namespace in "[a-z]{1,12}") {
        let mut db = Db::open("sqlite", ":memory:", &namespace).expect("open sqlite");
        db.execute("CREATE TABLE {ns}items(id INTEGER PRIMARY KEY)")
            .expect("create namespaced table");
        let expected = format!("{namespace}_items");
        let table = db
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
                &[Value::Text(expected.clone())],
            )
            .expect("query namespaced table");
        prop_assert_eq!(table, vec![Value::Text(expected)]);
    }
}
