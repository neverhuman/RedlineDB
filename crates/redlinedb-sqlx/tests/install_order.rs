use std::panic::{AssertUnwindSafe, catch_unwind};

use redlinedb_sqlx::install_default_drivers;

#[test]
fn upstream_install_blocks_late_redline_install() {
    static EMPTY: &[sqlx::any::driver::AnyDriver] = &[];

    sqlx::any::driver::install_drivers(EMPTY).expect("seed empty registry");

    let result = catch_unwind(AssertUnwindSafe(install_default_drivers));
    assert!(result.is_err(), "late driver install should panic");
}
