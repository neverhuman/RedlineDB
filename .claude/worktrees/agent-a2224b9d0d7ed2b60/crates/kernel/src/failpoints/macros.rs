//! `fail_point!` macro wrapper.
//!
//! Forwards to [`fail::fail_point!`] when the `failpoints` feature is enabled
//! and expands to nothing otherwise, so kernel hot paths can host injection
//! points at zero runtime cost in default builds.

/// Insert a named failpoint at the call site.
///
/// Two forms are supported:
///
/// ```ignore
/// use redlinedb_kernel::fail_point;
///
/// fail_point!("kernel::wal::after_fsync");
/// fail_point!("kernel::wal::before_fsync", |_arg| {
///     return Err(crate::error::Error::Io("injected fault".into()));
/// });
/// ```
///
/// When the `failpoints` feature is disabled both forms expand to nothing
/// and incur zero runtime cost.
#[cfg(feature = "failpoints")]
#[macro_export]
macro_rules! fail_point {
    ($name:expr) => {{
        $crate::__fail_reexport::fail_point!($name);
    }};
    ($name:expr, $closure:expr) => {{
        $crate::__fail_reexport::fail_point!($name, $closure);
    }};
}

/// No-op expansion of [`fail_point!`] when the feature is disabled.
#[cfg(not(feature = "failpoints"))]
#[macro_export]
macro_rules! fail_point {
    ($name:expr) => {{}};
    ($name:expr, $closure:expr) => {{}};
}
