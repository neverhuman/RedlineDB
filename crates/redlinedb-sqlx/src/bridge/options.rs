use super::runtime::{
    DEFAULT_BUSY_TIMEOUT, REDLINE_URL_SCHEMES, RedlineUrlMode, join_error, map_redline_error,
    parse_location, parse_mode,
};
#[allow(unused_imports)]
use super::*;

pub(crate) struct ConnectionState {
    #[allow(dead_code)]
    pub(crate) db: redlinedb::Database,
    #[allow(dead_code)]
    pub(crate) conn: redlinedb::Connection,
}

impl std::fmt::Debug for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionState").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct RedlineConnectOptions {
    pub(crate) database_url: Url,
    pub(crate) location: RedlineLocation,
    pub(crate) open_options: redlinedb::OpenOptions,
    pub(crate) busy_timeout: Duration,
}

#[derive(Clone, Debug)]
pub(crate) enum RedlineLocation {
    File(PathBuf),
    InMemory,
}

#[derive(Debug)]
pub struct RedlineConnection {
    pub(crate) state: Arc<Mutex<ConnectionState>>,
}

impl RedlineConnection {
    pub(crate) fn with_state<T>(
        state: Arc<Mutex<ConnectionState>>,
        f: impl FnOnce(&mut redlinedb::Connection) -> Result<T, Error> + Send + 'static,
    ) -> BoxFuture<'static, Result<T, Error>>
    where
        T: Send + 'static,
    {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut guard = state
                    .lock()
                    .map_err(|_| Error::Protocol("redline connection mutex poisoned".into()))?;
                f(&mut guard.conn)
            })
            .await
            .map_err(join_error)?
        })
    }

    fn connect_sync(options: RedlineConnectOptions) -> Result<Self, Error> {
        let RedlineConnectOptions {
            location,
            open_options,
            busy_timeout,
            ..
        } = options;

        let db = match location {
            RedlineLocation::File(path) => {
                redlinedb::Database::open_with_options(path, open_options.clone())
                    .map_err(map_redline_error)?
            }
            RedlineLocation::InMemory => {
                redlinedb::Database::create_in_memory(open_options.clone())
                    .map_err(map_redline_error)?
            }
        };
        let conn = db.connect().map_err(map_redline_error)?;

        let mut conn = conn;
        conn.set_busy_timeout(busy_timeout);

        Ok(Self {
            state: Arc::new(Mutex::new(ConnectionState { db, conn })),
        })
    }
}

impl ConnectOptions for RedlineConnectOptions {
    type Connection = RedlineConnection;

    fn from_url(url: &Url) -> Result<Self, Error> {
        if !REDLINE_URL_SCHEMES.contains(&url.scheme()) {
            return Err(Error::Configuration(
                format!(
                    "unsupported URL scheme {:?} for RedlineDB; expected redline:// or redlinedb://",
                    url.scheme()
                )
                .into(),
            ));
        }

        let mode = parse_mode(url)?;
        let location = parse_location(url)?;
        if matches!(location, RedlineLocation::InMemory) && mode == RedlineUrlMode::Ro {
            // HLT-022-AUTHZ-ISOLATION-GAP negative proof for the attach boundary:
            // crates/redlinedb-sqlx/tests/attach_mode.rs::mode_ro_attaches_to_live_database_and_blocks_writes
            // proves the non-owner attach path can read live rows, rejects writes,
            // and a second rwc opener still hits the owner lock.
            return Err(Error::Configuration(
                "read-only attach mode is only supported for file-backed RedlineDB URL values"
                    .into(),
            ));
        }

        let mut open_options = redlinedb::OpenOptions::default();
        match mode {
            RedlineUrlMode::Rwc => {
                open_options.create = true;
                open_options.read_only = false;
                open_options.process_owner_lock = true;
            }
            RedlineUrlMode::Ro => {
                open_options.create = false;
                open_options.read_only = true;
                open_options.process_owner_lock = false;
            }
        }

        Ok(Self {
            database_url: url.clone(),
            location,
            open_options,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        })
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, Error>> {
        let options = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || RedlineConnection::connect_sync(options))
                .await
                .map_err(join_error)?
        })
    }

    fn log_statements(self, _level: log::LevelFilter) -> Self {
        self
    }

    fn log_slow_statements(self, _level: log::LevelFilter, _duration: Duration) -> Self {
        self
    }

    fn to_url_lossy(&self) -> Url {
        self.database_url.clone()
    }
}

impl std::str::FromStr for RedlineConnectOptions {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s).map_err(Error::config)?;
        Self::from_url(&url)
    }
}

impl TryFrom<&AnyConnectOptions> for RedlineConnectOptions {
    type Error = Error;

    fn try_from(options: &AnyConnectOptions) -> Result<Self, Self::Error> {
        Self::from_url(&options.database_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_ro_disables_owner_lock_and_creation() {
        let url = Url::parse("redline:///tmp/attach.redlineDB?mode=ro").expect("url");
        let opts = RedlineConnectOptions::from_url(&url).expect("connect options");

        assert!(matches!(opts.location, RedlineLocation::File(_)));
        assert!(!opts.open_options.create);
        assert!(opts.open_options.read_only);
        assert!(!opts.open_options.process_owner_lock);
    }

    #[test]
    fn ro_is_rejected_for_in_memory_url() {
        let url = Url::parse("redline:///:memory:?mode=ro").expect("url");
        let err = RedlineConnectOptions::from_url(&url).expect_err("reject memory ro");

        assert!(
            err.to_string()
                .contains("read-only attach mode is only supported for file-backed"),
            "unexpected error: {err}"
        );
    }
}

impl Connection for RedlineConnection {
    type Database = RedlineDb;
    type Options = RedlineConnectOptions;

    fn close(self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn close_hard(self) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            drop(self);
            Ok(())
        })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        let state = Arc::clone(&self.state);
        Self::with_state(state, |_conn| Ok(()))
    }

    fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, None)
    }

    fn begin_with(
        &mut self,
        statement: impl Into<Cow<'static, str>>,
    ) -> BoxFuture<'_, Result<Transaction<'_, Self::Database>, Error>>
    where
        Self: Sized,
    {
        Transaction::begin(self, Some(statement.into()))
    }

    fn cached_statements_size(&self) -> usize {
        0
    }

    fn clear_cached_statements(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }

    fn shrink_buffers(&mut self) {}

    fn flush(&mut self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }

    fn should_flush(&self) -> bool {
        false
    }
}
