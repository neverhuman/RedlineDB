//! Runtime configuration, parsed from CLI flags and environment variables.
//!
//! Every field maps one-to-one to a row of the config table in `CONTRACT.md`.
//! Flags take precedence over their `REDLINE_WEB_*` environment fallbacks.

use std::net::SocketAddr;

use clap::Parser;

/// Sentinel `--db` value selecting an in-memory database seeded with demo data.
pub const MEMORY_DB: &str = ":memory:";

/// redline-web server configuration.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "redline-web",
    about = "Database observability + SQL console backend for redline-core or any SQLite database.",
    version
)]
pub struct Config {
    /// SQLite file to open (`:memory:` seeds a demo schema).
    #[arg(long, env = "REDLINE_WEB_DB", default_value = MEMORY_DB)]
    pub db: String,

    /// External redline/SQLite CLI to drive instead of opening a file directly.
    #[arg(long, env = "REDLINE_WEB_TARGET_BIN")]
    pub target_bin: Option<String>,

    /// Listen address.
    #[arg(long, env = "REDLINE_WEB_BIND", default_value = "127.0.0.1:7788")]
    pub bind: SocketAddr,

    /// Open the database read-only and reject writes through `/api/query`.
    #[arg(long, env = "REDLINE_WEB_READ_ONLY", default_value_t = false)]
    pub read_only: bool,

    /// Maximum rows returned by a query before truncation.
    #[arg(long, env = "REDLINE_WEB_MAX_ROWS", default_value_t = 1000)]
    pub max_rows: i64,

    /// Per-query timeout in milliseconds.
    #[arg(long, env = "REDLINE_WEB_QUERY_TIMEOUT_MS", default_value_t = 15_000)]
    pub query_timeout_ms: u64,

    /// Slow-query threshold in milliseconds (records into the slow-query ring).
    #[arg(long, env = "REDLINE_WEB_SLOW_MS", default_value_t = 100)]
    pub slow_ms: u64,
}

impl Config {
    /// True when the server should run against an in-memory demo database.
    pub fn is_memory(&self) -> bool {
        self.db == MEMORY_DB
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db: MEMORY_DB.to_string(),
            target_bin: None,
            bind: "127.0.0.1:7788".parse().expect("valid default bind addr"),
            read_only: false,
            max_rows: 1000,
            query_timeout_ms: 15_000,
            slow_ms: 100,
        }
    }
}
