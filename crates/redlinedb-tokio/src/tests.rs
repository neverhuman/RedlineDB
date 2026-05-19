use super::*;

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn pool_is_send_sync() {
    assert_send::<Pool>();
    assert_sync::<Pool>();
}

#[test]
fn async_row_is_send_sync() {
    assert_send::<AsyncRow>();
    assert_sync::<AsyncRow>();
}
