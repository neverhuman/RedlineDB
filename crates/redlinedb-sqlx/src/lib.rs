//! SQLx Any driver bridge for RedlineDB.
//!
//! Call [`install_default_drivers`] before the first `sqlx::AnyPool` or
//! `sqlx::AnyConnection` is created so the `redline://` URL scheme is
//! registered.

mod bridge;
mod dummy;

/// Install the RedlineDB driver into SQLx's `Any` registry.
///
/// Call this before the first `sqlx::AnyPool` or `sqlx::AnyConnection`
/// is created.
pub fn install_default_drivers() {
    bridge::install_redline_driver_once();
}
