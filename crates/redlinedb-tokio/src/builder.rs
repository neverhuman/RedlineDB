#![allow(missing_docs)]

use std::sync::Arc;
use std::time::Duration;

use crate::pool::PoolInner;
use crate::{
    DEFAULT_BUSY_TIMEOUT, DEFAULT_MAX_CONNECTIONS, Database, Error, ErrorCode, Pool, Result,
};

pub struct PoolBuilder {
    db: Option<Database>,
    max_connections: usize,
    busy_timeout: Duration,
}

impl Default for PoolBuilder {
    fn default() -> Self {
        Self {
            db: None,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }
}

impl PoolBuilder {
    pub fn database(mut self, db: Database) -> Self {
        self.db = Some(db);
        self
    }

    pub fn max_connections(mut self, max: usize) -> Self {
        assert!(max > 0, "max_connections must be > 0");
        self.max_connections = max;
        self
    }

    pub fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = timeout;
        self
    }

    pub fn build(self) -> Result<Pool> {
        let db = match self.db {
            Some(db) => db,
            None => {
                return Err(Error::new(
                    ErrorCode::Misuse,
                    "PoolBuilder requires a Database",
                ));
            }
        };
        Ok(Pool {
            inner: Arc::new(PoolInner {
                db,
                semaphore: Arc::new(tokio::sync::Semaphore::new(self.max_connections)),
                busy_timeout: self.busy_timeout,
                max_connections: self.max_connections,
            }),
        })
    }
}
