use db_shim::{Db, Error, SqlPart, Statement, Value};

#[test]
fn release_identity_is_native_redline_4_1_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "4.1.0");
}

#[test]
fn invalid_namespace_fails_before_connecting() {
    let error = match Db::open("unused", "orders;drop") {
        Ok(_) => panic!("invalid namespace must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Config(message) if message.contains("[a-z]")));
}

#[test]
fn parameterized_statement_owns_values() {
    let statement = Statement::compose([
        SqlPart::Syntax("SELECT "),
        SqlPart::Parameter(Value::Text("owned".to_owned())),
    ])
    .unwrap();
    assert_eq!(statement.params(), &[Value::Text("owned".to_owned())]);
}

#[cfg(feature = "oracle-sqlite")]
#[test]
fn sqlite_runs_the_governed_corpus() {
    let mut db = Db::open(":memory:", "oracle_test").expect("open SQLite oracle");
    let report = db_shim::corpus::run(&mut db).expect("run governed corpus");
    assert_eq!(report.schema_version, db_shim::corpus::VERSION);
    assert_eq!(report.rows, 3);
    assert!(report.rollback_preserved_rows);
}
