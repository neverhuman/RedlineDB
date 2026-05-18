use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::exit;
use std::thread;

use redlinedb::{Database, Value, ValueRef};
use serde::{Deserialize, Serialize};

const PROTOCOL_MAGIC: [u8; 4] = *b"RLDB";
const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Request {
    Hello,
    Prepare {
        stmt_id: u64,
        sql: String,
    },
    Bind {
        stmt_id: u64,
        values: Vec<WireValue>,
    },
    Step {
        stmt_id: u64,
        max_rows: usize,
    },
    Reset {
        stmt_id: u64,
    },
    Finalize {
        stmt_id: u64,
    },
    Exec {
        sql: String,
    },
    Begin {
        mode: Option<String>,
    },
    Commit,
    Rollback,
    Interrupt,
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
        rows: Vec<Vec<WireValue>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
enum WireValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl WireValue {
    fn from_value_ref(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(value) => Self::Integer(value),
            ValueRef::Real(value) => Self::Real(value),
            ValueRef::Text(value) => Self::Text(value.to_owned()),
            ValueRef::Blob(value) => Self::Blob(value.to_vec()),
        }
    }

    fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Integer(value) => Value::Integer(value),
            Self::Real(value) => Value::Real(value),
            Self::Text(value) => Value::Text(value.into()),
            Self::Blob(value) => Value::Blob(value.into()),
        }
    }
}

struct ActiveStatement {
    id: u64,
    stmt: redlinedb::OwnedStatement,
}

fn main() {
    let mut args = env::args().skip(1);
    let mut database = None;
    let mut listen = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--database" => database = args.next(),
            "--listen" => listen = args.next(),
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                exit(2);
            }
        }
    }

    let database = database.unwrap_or_else(|| {
        eprintln!("missing --database");
        exit(2);
    });
    let listen = listen.unwrap_or_else(|| {
        eprintln!("missing --listen");
        exit(2);
    });
    let db = Database::open(database).unwrap_or_else(|err| {
        eprintln!("{err}");
        exit(1);
    });
    let listener = TcpListener::bind(listen).unwrap_or_else(|err| {
        eprintln!("{err}");
        exit(1);
    });
    serve(listener, db);
}

fn serve(listener: TcpListener, db: Database) {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };
        let db = db.clone();
        thread::spawn(move || handle_client(stream, db));
    }
}

fn handle_client(mut stream: TcpStream, db: Database) {
    if send_magic(&mut stream).is_err() || read_magic(&mut stream).is_err() {
        return;
    }

    let mut conn = match db.connect() {
        Ok(conn) => conn,
        Err(err) => {
            let _ = write_response(
                &mut stream,
                &Response::Error {
                    code: err.code() as i32,
                    message: err.to_string(),
                },
            );
            return;
        }
    };

    let mut active: Option<ActiveStatement> = None;
    loop {
        let request = match read_request(&mut stream) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(err) => {
                let _ = write_response(
                    &mut stream,
                    &Response::Error {
                        code: -1,
                        message: err.to_string(),
                    },
                );
                break;
            }
        };

        let response = match request {
            Request::Hello => Response::Hello {
                protocol_version: PROTOCOL_VERSION,
                server: "redlinedb-server".to_owned(),
            },
            Request::Prepare { stmt_id, sql } => {
                active = None;
                match conn.prepare_owned(&sql) {
                    Ok(stmt) => {
                        let readonly = stmt.is_readonly();
                        let parameter_count = stmt.parameter_count();
                        let column_count = stmt.column_count();
                        let columns = (0..column_count)
                            .map(|index| stmt.column_name(index).to_owned())
                            .collect();
                        active = Some(ActiveStatement { id: stmt_id, stmt });
                        Response::Prepared {
                            stmt_id,
                            readonly,
                            parameter_count,
                            column_count,
                            columns,
                        }
                    }
                    Err(err) => error_response(err),
                }
            }
            Request::Bind { stmt_id, values } => match active.as_mut() {
                Some(active_stmt) if active_stmt.id == stmt_id => {
                    let result = bind_values(&mut active_stmt.stmt, values);
                    match result {
                        Ok(()) => Response::Bound {
                            stmt_id,
                            parameter_count: active_stmt.stmt.parameter_count(),
                        },
                        Err(err) => error_response(err),
                    }
                }
                Some(_) => Response::Error {
                    code: 21,
                    message: "statement id mismatch".to_owned(),
                },
                None => Response::Error {
                    code: 21,
                    message: "no active statement".to_owned(),
                },
            },
            Request::Step { stmt_id, max_rows } => match active.as_mut() {
                Some(active_stmt) if active_stmt.id == stmt_id => {
                    match step_rows(&mut active_stmt.stmt, max_rows.max(1)) {
                        Ok((rows, done)) => Response::Rows {
                            stmt_id,
                            rows,
                            done,
                        },
                        Err(err) => error_response(err),
                    }
                }
                Some(_) => Response::Error {
                    code: 21,
                    message: "statement id mismatch".to_owned(),
                },
                None => Response::Error {
                    code: 21,
                    message: "no active statement".to_owned(),
                },
            },
            Request::Reset { stmt_id } => match active.as_mut() {
                Some(active_stmt) if active_stmt.id == stmt_id => match active_stmt.stmt.reset() {
                    Ok(()) => Response::Ok,
                    Err(err) => error_response(err),
                },
                Some(_) => Response::Error {
                    code: 21,
                    message: "statement id mismatch".to_owned(),
                },
                None => Response::Error {
                    code: 21,
                    message: "no active statement".to_owned(),
                },
            },
            Request::Finalize { stmt_id } => match active.take() {
                Some(active_stmt) if active_stmt.id == stmt_id => Response::Ok,
                Some(active_stmt) => {
                    active = Some(active_stmt);
                    Response::Error {
                        code: 21,
                        message: "statement id mismatch".to_owned(),
                    }
                }
                None => Response::Error {
                    code: 21,
                    message: "no active statement".to_owned(),
                },
            },
            Request::Exec { sql } => {
                if active.is_some() {
                    Response::Error {
                        code: 5,
                        message: "finalize the active statement first".to_owned(),
                    }
                } else {
                    match conn.execute(&sql, ()) {
                        Ok(summary) => Response::Summary {
                            rows_affected: summary.rows_affected,
                            rows_returned: summary.rows_returned,
                        },
                        Err(err) => error_response(err),
                    }
                }
            }
            Request::Begin { mode } => {
                if active.is_some() {
                    Response::Error {
                        code: 5,
                        message: "finalize the active statement first".to_owned(),
                    }
                } else {
                    let mode = match mode.as_deref() {
                        Some("immediate") => redlinedb::BeginMode::Immediate,
                        Some("exclusive") => redlinedb::BeginMode::Exclusive,
                        _ => redlinedb::BeginMode::Deferred,
                    };
                    match conn.begin(mode) {
                        Ok(()) => Response::Ok,
                        Err(err) => error_response(err),
                    }
                }
            }
            Request::Commit => {
                if active.is_some() {
                    Response::Error {
                        code: 5,
                        message: "finalize the active statement first".to_owned(),
                    }
                } else {
                    match conn.commit() {
                        Ok(_) => Response::Ok,
                        Err(err) => error_response(err),
                    }
                }
            }
            Request::Rollback => {
                if active.is_some() {
                    Response::Error {
                        code: 5,
                        message: "finalize the active statement first".to_owned(),
                    }
                } else {
                    match conn.rollback() {
                        Ok(()) => Response::Ok,
                        Err(err) => error_response(err),
                    }
                }
            }
            Request::Interrupt => {
                db.interrupt_all();
                Response::Ok
            }
            Request::Close => {
                let _ = active.take();
                let _ = write_response(&mut stream, &Response::Ok);
                break;
            }
        };

        if write_response(&mut stream, &response).is_err() {
            break;
        }
    }
}

fn step_rows(
    stmt: &mut redlinedb::OwnedStatement,
    max_rows: usize,
) -> redlinedb::Result<(Vec<Vec<WireValue>>, bool)> {
    let mut rows = Vec::new();
    let mut done = false;
    let column_count = stmt.column_count();
    while rows.len() < max_rows {
        match stmt.step()? {
            redlinedb::OwnedStep::Row => {
                let mut values = Vec::with_capacity(column_count);
                for index in 0..column_count {
                    values.push(WireValue::from_value_ref(stmt.column_ref(index)?));
                }
                rows.push(values);
            }
            redlinedb::OwnedStep::Done => {
                done = true;
                break;
            }
        }
    }
    Ok((rows, done))
}

fn bind_values(
    stmt: &mut redlinedb::OwnedStatement,
    values: Vec<WireValue>,
) -> redlinedb::Result<()> {
    stmt.clear_bindings();
    for (index, value) in values.into_iter().enumerate() {
        stmt.bind_value(index + 1, value.into_value())?;
    }
    Ok(())
}

fn error_response(err: redlinedb::Error) -> Response {
    Response::Error {
        code: err.code() as i32,
        message: err.to_string(),
    }
}

fn send_magic(stream: &mut TcpStream) -> std::io::Result<()> {
    stream.write_all(&PROTOCOL_MAGIC)?;
    stream.write_all(&PROTOCOL_VERSION.to_be_bytes())?;
    stream.write_all(&0u16.to_be_bytes())?;
    stream.flush()
}

fn read_magic(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0_u8; 8];
    stream.read_exact(&mut buf)?;
    if buf[0..4] != PROTOCOL_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "protocol magic mismatch",
        ));
    }
    let version = u16::from_be_bytes([buf[4], buf[5]]);
    if version != PROTOCOL_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "protocol version mismatch",
        ));
    }
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut len_buf = [0_u8; 4];
    match stream.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0_u8; len];
    stream.read_exact(&mut payload)?;
    let request = serde_json::from_slice(&payload)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    Ok(Some(request))
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let payload = serde_json::to_vec(response)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err.to_string()))?;
    stream.write_all(&(payload.len() as u32).to_be_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()
}

fn print_help() {
    println!("redlinedb-server --database FILE --listen HOST:PORT");
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;

    use tempfile::tempdir;

    use super::*;

    fn start_server() -> (tempfile::TempDir, String) {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("server.redline");
        let db = Database::open(&db_path).expect("open db");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("local addr").to_string();
        thread::spawn(move || serve(listener, db));
        (dir, addr)
    }

    fn connect(addr: &str) -> TcpStream {
        let mut stream = TcpStream::connect(addr).expect("connect");
        let mut header = [0_u8; 8];
        stream.read_exact(&mut header).expect("server header");
        assert_eq!(&header[0..4], b"RLDB");
        stream.write_all(&header).expect("client header");
        stream
    }

    fn roundtrip(stream: &mut TcpStream, request: &Request) -> Response {
        let payload = serde_json::to_vec(request).expect("serialize");
        stream
            .write_all(&(payload.len() as u32).to_be_bytes())
            .expect("len");
        stream.write_all(&payload).expect("payload");
        stream.flush().expect("flush");
        let mut len_buf = [0_u8; 4];
        stream.read_exact(&mut len_buf).expect("resp len");
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0_u8; len];
        stream.read_exact(&mut payload).expect("resp payload");
        serde_json::from_slice(&payload).expect("response")
    }

    #[test]
    fn framed_protocol_executes_statements() {
        let (_dir, addr) = start_server();
        let mut stream = connect(&addr);

        assert!(matches!(
            roundtrip(&mut stream, &Request::Hello),
            Response::Hello { .. }
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Exec {
                    sql: "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)".to_owned(),
                }
            ),
            Response::Summary { .. }
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Prepare {
                    stmt_id: 7,
                    sql: "SELECT name FROM t WHERE id = ?1".to_owned(),
                }
            ),
            Response::Prepared { stmt_id: 7, .. }
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Bind {
                    stmt_id: 7,
                    values: vec![WireValue::Integer(1)],
                }
            ),
            Response::Bound { stmt_id: 7, .. }
        ));
        assert!(matches!(
            roundtrip(&mut stream, &Request::Step { stmt_id: 7, max_rows: 10 }),
            Response::Rows { rows, done: true, .. } if rows.is_empty()
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Exec {
                    sql: "INSERT INTO t(id, name) VALUES (1, 'Ada')".to_owned(),
                }
            ),
            Response::Error { .. }
        ));
        assert!(matches!(
            roundtrip(&mut stream, &Request::Finalize { stmt_id: 7 }),
            Response::Ok
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Exec {
                    sql: "INSERT INTO t(id, name) VALUES (1, 'Ada')".to_owned(),
                }
            ),
            Response::Summary { .. }
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Prepare {
                    stmt_id: 8,
                    sql: "SELECT name FROM t WHERE id = ?1".to_owned(),
                }
            ),
            Response::Prepared { .. }
        ));
        assert!(matches!(
            roundtrip(
                &mut stream,
                &Request::Bind {
                    stmt_id: 8,
                    values: vec![WireValue::Integer(1)],
                }
            ),
            Response::Bound { .. }
        ));
        assert!(matches!(
            roundtrip(&mut stream, &Request::Step { stmt_id: 8, max_rows: 10 }),
            Response::Rows { rows, done: true, .. }
                if rows.len() == 1 && matches!(rows[0][0], WireValue::Text(_))
        ));
        assert!(matches!(
            roundtrip(&mut stream, &Request::Close),
            Response::Ok
        ));
    }
}
