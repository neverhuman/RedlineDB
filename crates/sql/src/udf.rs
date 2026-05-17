//! Pluggable user-defined function dispatch hook.
//!
//! The FFI layer (`redlinedb-ffi`) installs a dispatcher via
//! [`install_dispatch`] when `sqlite3_create_function*` is first called.
//! The SQL expression evaluator consults [`call_registered_scalar`] for any
//! function name it does not recognise — if a registered UDF matches, the
//! evaluator returns its result instead of `Error::UnsupportedSql`.
//!
//! The hook receives the originating db address as `usize` so the FFI side
//! can scope registrations per `*mut sqlite3` connection.

use std::sync::OnceLock;

use crate::value::SqlValue;

pub type DispatchFn =
    fn(db_addr: usize, name: &str, args: &[SqlValue]) -> Option<Result<SqlValue, String>>;

static DISPATCH: OnceLock<DispatchFn> = OnceLock::new();

/// Install the global UDF dispatcher. Idempotent — calling more than once
/// returns `false` and does not replace the existing dispatcher.
pub fn install_dispatch(f: DispatchFn) -> bool {
    DISPATCH.set(f).is_ok()
}

/// Consult the dispatcher (if installed). Returns `Some(Ok(value))` on a
/// successful UDF call, `Some(Err(msg))` if the UDF returned an error,
/// `None` if no UDF is registered for that name.
///
/// `db_addr` carries the connection identity (typically the `*mut sqlite3`
/// pointer cast to `usize`) so registrations can be scoped per connection.
pub fn call_registered_scalar(
    db_addr: usize,
    name: &str,
    args: &[SqlValue],
) -> Option<Result<SqlValue, String>> {
    DISPATCH.get().and_then(|f| f(db_addr, name, args))
}

/// Pluggable collation dispatcher analogous to [`install_dispatch`] but for
/// custom text collations registered via `sqlite3_create_collation*`.
pub type CollationFn = fn(db_addr: usize, name: &str, a: &str, b: &str) -> Option<std::cmp::Ordering>;

static COLLATION: OnceLock<CollationFn> = OnceLock::new();

pub fn install_collation_dispatch(f: CollationFn) -> bool {
    COLLATION.set(f).is_ok()
}

pub fn call_registered_collation(
    db_addr: usize,
    name: &str,
    a: &str,
    b: &str,
) -> Option<std::cmp::Ordering> {
    COLLATION.get().and_then(|f| f(db_addr, name, a, b))
}

/// Current connection address. Set per-statement by the FFI prepare path so
/// the dispatchers can scope lookups. Defaults to `0` when no FFI caller is
/// active (pure-Rust use).
use std::cell::Cell;
thread_local! {
    static CURRENT_DB: Cell<usize> = const { Cell::new(0) };
}

pub fn current_db() -> usize {
    CURRENT_DB.with(|c| c.get())
}

pub fn with_db<R>(addr: usize, f: impl FnOnce() -> R) -> R {
    CURRENT_DB.with(|c| {
        let prev = c.get();
        c.set(addr);
        let result = f();
        c.set(prev);
        result
    })
}
