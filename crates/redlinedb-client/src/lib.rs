//! Sync remote client for `redlinedb-server` (redline-core `crates/server`).
//!
//! The server exposes a framed TCP protocol: an 8-byte magic handshake, then length-prefixed
//! (u32 big-endian) serde_json request/response frames, driving the redlinedb statement lifecycle
//! (prepare → bind → step → finalize, plus exec / begin / commit / rollback). This client speaks
//! that protocol and presents a small rusqlite-shaped surface so the `db-shim` (and consumers) can
//! target the central server the same way they target an embedded database.
//!
//! Wire types mirror `redline-core/crates/server/src/main.rs` verbatim (same serde tags), so the
//! JSON is byte-compatible with the server. The canonical home for these types is a future
//! `redlinedb-wire` crate shared by server + client; they are declared here until that lands.

use std::io::{self, Read, Write};
use std::net::TcpStream;

use serde::{Deserialize, Serialize};

const PROTOCOL_MAGIC: [u8; 4] = *b"RLDB";
const PROTOCOL_VERSION: u16 = 1;

/// A SQL value on the wire (mirrors the server's `WireValue` + redlinedb `Value`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Hello,
    Prepare { stmt_id: u64, sql: String },
    Bind { stmt_id: u64, values: Vec<Value> },
    Step { stmt_id: u64, max_rows: usize },
    Finalize { stmt_id: u64 },
    Exec { sql: String },
    Begin { mode: Option<String> },
    Commit,
    Rollback,
    Close,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Response {
    Hello {
        protocol_version: u16,
        server: String,
    },
    Prepared {
        stmt_id: u64,
        readonly: bool,
        parameter_count: usize,
        column_count: usize,
        columns: Vec<String>,
    },
    Bound {
        stmt_id: u64,
        parameter_count: usize,
    },
    Rows {
        stmt_id: u64,
        rows: Vec<Vec<Value>>,
        done: bool,
    },
    Summary {
        rows_affected: u64,
        rows_returned: u64,
    },
    Ok,
    Error {
        code: i32,
        message: String,
    },
}

/// A client-side error: an I/O failure, a protocol violation, or a server-reported SQL error.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Protocol(String),
    Server { code: i32, message: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io: {e}"),
            Error::Protocol(m) => write!(f, "protocol: {m}"),
            Error::Server { code, message } => write!(f, "server[{code}]: {message}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// A single result column set from a query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

/// A synchronous connection to a `redlinedb-server`.
pub struct Client {
    stream: TcpStream,
    next_stmt_id: u64,
}

impl Client {
    /// Connect to `addr` (e.g. `"127.0.0.1:6033"`), perform the magic handshake, and verify the
    /// server's protocol version via `Hello`.
    pub fn connect(addr: &str) -> Result<Self> {
        let mut stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true).ok();

        // The server sends its magic first, then reads ours.
        let mut server_magic = [0u8; 8];
        stream.read_exact(&mut server_magic)?;
        if server_magic[0..4] != PROTOCOL_MAGIC {
            return Err(Error::Protocol("server magic mismatch".into()));
        }
        let server_version = u16::from_be_bytes([server_magic[4], server_magic[5]]);
        if server_version != PROTOCOL_VERSION {
            return Err(Error::Protocol(format!(
                "server protocol version {server_version} != {PROTOCOL_VERSION}"
            )));
        }
        let mut client_magic = [0u8; 8];
        client_magic[0..4].copy_from_slice(&PROTOCOL_MAGIC);
        client_magic[4..6].copy_from_slice(&PROTOCOL_VERSION.to_be_bytes());
        stream.write_all(&client_magic)?;
        stream.flush()?;

        let mut client = Client {
            stream,
            next_stmt_id: 1,
        };
        match client.request(&Request::Hello)? {
            Response::Hello { .. } => Ok(client),
            other => Err(Error::Protocol(format!(
                "unexpected hello response: {other:?}"
            ))),
        }
    }

    fn request(&mut self, req: &Request) -> Result<Response> {
        let payload = serde_json::to_vec(req).map_err(|e| Error::Protocol(e.to_string()))?;
        let len = u32::try_from(payload.len())
            .map_err(|_| Error::Protocol("request too large".into()))?;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(&payload)?;
        self.stream.flush()?;

        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        self.stream.read_exact(&mut resp_buf)?;
        let resp: Response =
            serde_json::from_slice(&resp_buf).map_err(|e| Error::Protocol(e.to_string()))?;
        if let Response::Error { code, message } = resp {
            return Err(Error::Server { code, message });
        }
        Ok(resp)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_stmt_id;
        self.next_stmt_id = self.next_stmt_id.wrapping_add(1);
        id
    }

    /// Execute a non-parameterized statement (DDL/DML). Returns rows affected.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        match self.request(&Request::Exec {
            sql: sql.to_owned(),
        })? {
            Response::Summary { rows_affected, .. } => Ok(rows_affected),
            other => Err(Error::Protocol(format!("exec: unexpected {other:?}"))),
        }
    }

    /// Execute a parameterized statement (prepare → bind → step to completion → finalize).
    pub fn execute_params(&mut self, sql: &str, params: &[Value]) -> Result<()> {
        let (id, _cols) = self.prepare(sql)?;
        self.bind(id, params)?;
        loop {
            let (_rows, done) = self.step(id, 512)?;
            if done {
                break;
            }
        }
        self.finalize(id)
    }

    /// Run a query (prepare → bind → step-until-done → finalize) and collect all rows.
    pub fn query(&mut self, sql: &str, params: &[Value]) -> Result<QueryResult> {
        let (id, columns) = self.prepare(sql)?;
        if !params.is_empty() {
            self.bind(id, params)?;
        }
        let mut rows = Vec::new();
        loop {
            let (mut batch, done) = self.step(id, 512)?;
            rows.append(&mut batch);
            if done {
                break;
            }
        }
        self.finalize(id)?;
        Ok(QueryResult { columns, rows })
    }

    /// Query expecting exactly one row; errors if none.
    pub fn query_row(&mut self, sql: &str, params: &[Value]) -> Result<Vec<Value>> {
        let mut r = self.query(sql, params)?;
        if r.rows.is_empty() {
            return Err(Error::Server {
                code: 12,
                message: "query returned no rows".into(),
            });
        }
        Ok(r.rows.remove(0))
    }

    fn prepare(&mut self, sql: &str) -> Result<(u64, Vec<String>)> {
        let id = self.next_id();
        match self.request(&Request::Prepare {
            stmt_id: id,
            sql: sql.to_owned(),
        })? {
            Response::Prepared {
                stmt_id, columns, ..
            } => Ok((stmt_id, columns)),
            other => Err(Error::Protocol(format!("prepare: unexpected {other:?}"))),
        }
    }

    fn bind(&mut self, stmt_id: u64, params: &[Value]) -> Result<()> {
        match self.request(&Request::Bind {
            stmt_id,
            values: params.to_vec(),
        })? {
            Response::Bound { .. } => Ok(()),
            other => Err(Error::Protocol(format!("bind: unexpected {other:?}"))),
        }
    }

    fn step(&mut self, stmt_id: u64, max_rows: usize) -> Result<(Vec<Vec<Value>>, bool)> {
        match self.request(&Request::Step { stmt_id, max_rows })? {
            Response::Rows { rows, done, .. } => Ok((rows, done)),
            other => Err(Error::Protocol(format!("step: unexpected {other:?}"))),
        }
    }

    fn finalize(&mut self, stmt_id: u64) -> Result<()> {
        match self.request(&Request::Finalize { stmt_id })? {
            Response::Ok => Ok(()),
            other => Err(Error::Protocol(format!("finalize: unexpected {other:?}"))),
        }
    }

    /// Begin a transaction (`mode`: None=deferred, "immediate", "exclusive").
    pub fn begin(&mut self, mode: Option<&str>) -> Result<()> {
        self.expect_ok(Request::Begin {
            mode: mode.map(|m| m.to_owned()),
        })
    }
    pub fn commit(&mut self) -> Result<()> {
        self.expect_ok(Request::Commit)
    }
    pub fn rollback(&mut self) -> Result<()> {
        self.expect_ok(Request::Rollback)
    }

    /// Run `op` inside a transaction; commit on Ok, rollback on Err.
    pub fn transaction<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.begin(Some("immediate"))?;
        match op(self) {
            Ok(v) => {
                self.commit()?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.rollback();
                Err(e)
            }
        }
    }

    fn expect_ok(&mut self, req: Request) -> Result<()> {
        match self.request(&req)? {
            Response::Ok => Ok(()),
            other => Err(Error::Protocol(format!("expected ok, got {other:?}"))),
        }
    }

    /// Politely close the session.
    pub fn close(mut self) {
        let _ = self.request(&Request::Close);
    }
}
