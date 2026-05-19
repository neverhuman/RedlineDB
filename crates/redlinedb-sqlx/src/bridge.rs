//! SQLx Any driver bridge for RedlineDB.
//!
//! Call [`install_default_drivers`] before the first `sqlx::AnyPool` or
//! `sqlx::AnyConnection` is created so the `redline://` and `redlinedb://`
//! URL schemes are registered.
//!
//! Canonical owning/server URL:
//! `redline:///absolute/path/to/target/jeryu/autonomy.redlineDB?mode=rwc`.
//! Canonical dashboard/TUI/inspection URL:
//! `redline:///absolute/path/to/target/jeryu/autonomy.redlineDB?mode=ro`.
//! HLT-022-AUTHZ-ISOLATION-GAP negative proof for the attach boundary:
//! `crates/redlinedb-sqlx/tests/attach_mode.rs::mode_ro_attaches_to_live_database_and_blocks_writes`
//! proves the non-owner attach path can read live rows, rejects writes,
//! and a second rwc opener still hits the owner lock; the companion
//! `ro_attach_is_rejected_for_in_memory_urls` case keeps `:memory:` out of
//! that attach path.
//! `redlinedb://` remains a compatibility alias, and URLs without `mode`
//! continue to behave as `mode=rwc`.

#[allow(unused_imports)]
use std::borrow::Cow;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use std::sync::{Arc, Mutex, Once};
#[allow(unused_imports)]
use std::time::Duration;

#[allow(unused_imports)]
use crate::driver::RedlineDb;
#[allow(unused_imports)]
use futures_core::future::BoxFuture;
#[allow(unused_imports)]
use futures_core::stream::BoxStream;
#[allow(unused_imports)]
use futures_util::{StreamExt, TryStreamExt, stream};
#[allow(unused_imports)]
use sqlx::Either;
#[allow(unused_imports)]
use sqlx::HashMap;
#[allow(unused_imports)]
use sqlx::any::{
    self, Any, AnyColumn, AnyConnectOptions, AnyConnectionBackend, AnyQueryResult, AnyRow,
    AnyStatement, AnyTypeInfo, AnyTypeInfoKind, AnyValue, AnyValueKind,
};
#[allow(unused_imports)]
use sqlx::connection::{ConnectOptions, Connection};
#[allow(unused_imports)]
use sqlx::describe::Describe;
#[allow(unused_imports)]
use sqlx::error::Error;
#[allow(unused_imports)]
use sqlx::ext::ustr::UStr;
#[allow(unused_imports)]
use sqlx::transaction::Transaction;
use url::Url;

mod options;
mod runtime;
#[cfg(test)]
mod tests;

pub(crate) use options::*;
pub(crate) use runtime::*;
