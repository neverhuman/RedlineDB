//! Minimal loopback HTTP fixture for the host-CI credential-boundary test.
//! It deliberately does not know the publisher token: the test can therefore
//! prove that no untrusted process learned it from the fake server itself.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let address_file = args.next().ok_or("missing address file")?;
    let log_file = args.next().ok_or("missing log file")?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    fs::write(&address_file, format!("http://{address}\n"))?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle(stream, Path::new(&log_file)) {
                    eprintln!("fake forge request error: {error}");
                }
            }
            Err(error) => eprintln!("fake forge accept error: {error}"),
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream, log_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut header_end = None;
    let mut content_length = 0_usize;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if header_end.is_none() {
            if let Some(index) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                header_end = Some(index + 4);
                let headers = String::from_utf8_lossy(&request[..index + 4]);
                content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
            }
        }
        if let Some(end) = header_end {
            if request.len() >= end + content_length {
                break;
            }
        }
        if request.len() > 1_048_576 {
            return Err("request too large".into());
        }
    }

    let text = String::from_utf8_lossy(&request);
    let request_line = text.lines().next().unwrap_or("");
    let auth_present = text.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("authorization: bearer ")
    });
    let body = header_end
        .and_then(|end| request.get(end..end.saturating_add(content_length)))
        .map(|bytes| String::from_utf8_lossy(bytes).replace(['\r', '\n'], " "))
        .unwrap_or_default();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(log, "{request_line}\tauth={auth_present}\tbody={body}")?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")?;
    Ok(())
}
