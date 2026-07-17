//! Run the governed used-operation corpus against the compile-time selected adapter.

use db_shim::{corpus, Db};

fn main() {
    let result = Db::from_env().and_then(|mut db| corpus::run(&mut db));
    match result {
        Ok(report) => println!(
            "DB-SHIM CORPUS PASS (schema={} rows={} rollback_preserved={} typed_null_round_trip={} execute_success_only={})",
            report.schema_version,
            report.rows,
            report.rollback_preserved_rows,
            report.typed_null_round_trip,
            report.execute_success_only
        ),
        Err(error) => {
            eprintln!("DB-SHIM CORPUS FAIL: {error}");
            std::process::exit(1);
        }
    }
}
