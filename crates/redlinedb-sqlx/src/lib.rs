//! SQLx Any driver bridge for RedlineDB.
//!
//! Call [`install_default_drivers`] before the first `sqlx::AnyPool` or
//! `sqlx::AnyConnection` is created so the `redline://` and `redlinedb://`
//! URL schemes are registered.
//!
//! For Jeryu autonomy ledgers, prefer
//! `redline:///absolute/path/to/target/jeryu/autonomy.redlineDB`.
//! `redlineDB:///absolute/path/to/target/jeryu/autonomy.redlineDB` is accepted
//! as a compatibility alias; URL parsing normalizes it to `redlinedb://`.

mod bridge;
mod dummy;

/// Install the RedlineDB driver into SQLx's `Any` registry.
///
/// Call this before the first `sqlx::AnyPool` or `sqlx::AnyConnection`
/// is created.
pub fn install_default_drivers() {
    bridge::install_redline_driver_once();
}
