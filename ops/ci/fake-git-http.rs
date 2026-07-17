//! Authenticated loopback smart-HTTP Git fixture for the Split Ops transport test.
//!
//! The fixture never logs an Authorization value. It accepts only one synthetic
//! Basic credential read from an explicit file and delegates accepted requests
//! to Git's CGI `http-backend` over a fixture-only repository root.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;

unsafe extern "C" {
    fn geteuid() -> u32;
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let project_root = args.next().ok_or("missing project root")?;
    let token_file = args.next().ok_or("missing token file")?;
    let log_file = args.next().ok_or("missing log file")?;
    let ready_file = args.next().ok_or("missing ready file")?;
    let auth_wait_file = args.next().ok_or("missing auth-wait file")?;
    let continue_file = args.next().ok_or("missing continue file")?;
    let behavior_file = args.next();
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    let token_path = Path::new(&token_file);
    if !token_path.is_absolute() {
        return Err("fixture token path is not absolute".into());
    }
    let token_metadata = fs::symlink_metadata(token_path)?;
    if !token_metadata.file_type().is_file()
        || token_metadata.uid() != unsafe { geteuid() }
        || token_metadata.mode() & 0o777 != 0o600
        || token_metadata.nlink() != 1
    {
        return Err("fixture token file metadata is unsafe".into());
    }
    let token = fs::read_to_string(token_path)?;
    let token = token.trim_end_matches(['\r', '\n']);
    if token.is_empty() || token.len() > 4096 {
        return Err("invalid fixture token length".into());
    }
    let expected_authorization = format!(
        "Basic {}",
        base64(format!("x-access-token:{token}").as_bytes())
    );

    let listener = TcpListener::bind("127.0.0.1:8787")?;
    fs::write(&ready_file, b"ready\n")?;
    let mut held_authenticated_request = false;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let request = read_request(&mut stream)?;
        if header(&request.headers, "host") != Some("127.0.0.1:8787")
            || !valid_git_target(&request.method, &request.target)
        {
            return Err("request escaped the fixed fixture origin or repository".into());
        }
        let authorization = header(&request.headers, "authorization");
        let auth_state = if authorization.is_none() {
            "absent"
        } else if authorization == Some(expected_authorization.as_str()) {
            "valid"
        } else {
            "invalid"
        };
        append_log(
            Path::new(&log_file),
            &format!("{} {} auth={auth_state}", request.method, request.target),
        )?;
        if auth_state != "valid" {
            respond_unauthorized(&mut stream)?;
            continue;
        }

        if !held_authenticated_request {
            held_authenticated_request = true;
            fs::write(&auth_wait_file, b"authenticated-request-held\n")?;
            let deadline = Instant::now() + Duration::from_secs(15);
            while !Path::new(&continue_file).is_file() {
                if Instant::now() >= deadline {
                    return Err("timed out waiting for process-boundary probe".into());
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        if let Some(behavior_file) = behavior_file.as_deref() {
            let behavior_path = Path::new(behavior_file);
            let behavior = fs::read_to_string(behavior_path).unwrap_or_default();
            if request.method == "POST" && request.target.ends_with("/git-upload-pack") {
                if behavior.trim() == "fail-upload-pack" {
                    respond_internal_error(&mut stream)?;
                    continue;
                }
                if let Some(rest) = behavior.trim().strip_prefix("move-ref ") {
                    let mut fields = rest.split(' ');
                    let reference = fields.next().ok_or("move-ref is missing a reference")?;
                    let new_head = fields.next().ok_or("move-ref is missing a new head")?;
                    let old_head = fields.next().ok_or("move-ref is missing an old head")?;
                    if fields.next().is_some()
                        || !reference.starts_with("refs/heads/")
                        || !valid_sha(new_head)
                        || !valid_sha(old_head)
                    {
                        return Err("invalid move-ref behavior".into());
                    }
                    let repository = Path::new(&project_root).join("jeryu/example.git");
                    let status = Command::new("/usr/bin/git")
                        .env_clear()
                        .env("PATH", "/usr/bin:/bin")
                        .env("LC_ALL", "C")
                        .arg(format!("--git-dir={}", repository.display()))
                        .args(["update-ref", reference, new_head, old_head])
                        .status()?;
                    if !status.success() {
                        return Err("fixture could not move the advertised ref".into());
                    }
                    fs::remove_file(behavior_path)?;
                }
            }
        }
        respond_from_git_backend(&mut stream, Path::new(&project_root), &request)?;
    }
    Ok(())
}

struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<Request, Box<dyn std::error::Error>> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut header_end = None;
    let mut content_length = 0_usize;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("fixture request is too large".into());
        }
        if header_end.is_none() {
            if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let end = index + 4;
                let headers = std::str::from_utf8(&bytes[..index])?;
                content_length = headers
                    .lines()
                    .skip(1)
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if content_length > MAX_REQUEST_BYTES.saturating_sub(end) {
                    return Err("fixture request body is too large".into());
                }
                header_end = Some(end);
            }
        }
        if header_end.is_some_and(|end| bytes.len() >= end + content_length) {
            break;
        }
    }

    let end = header_end.ok_or("fixture request has no complete header")?;
    if bytes.len() != end + content_length {
        return Err("fixture request body length mismatch".into());
    }
    let header_text = std::str::from_utf8(&bytes[..end - 4])?;
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing request method")?.to_owned();
    let target = parts.next().ok_or("missing request target")?.to_owned();
    if parts.next() != Some("HTTP/1.1") || parts.next().is_some() {
        return Err("unsupported request line".into());
    }
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or("malformed request header")?;
            Ok((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    Ok(Request {
        method,
        target,
        headers,
        body: bytes[end..].to_vec(),
    })
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_str())
}

fn valid_git_target(method: &str, target: &str) -> bool {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match (method, path, query) {
        (
            "GET",
            "/git/jeryu/example.git/info/refs",
            "service=git-upload-pack" | "service=git-receive-pack",
        ) => true,
        ("POST", "/git/jeryu/example.git/git-upload-pack", "")
        | ("POST", "/git/jeryu/example.git/git-receive-pack", "") => true,
        _ => false,
    }
}

fn respond_unauthorized(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    write!(
        stream,
        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"jeryu\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    Ok(())
}

fn respond_internal_error(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    write!(
        stream,
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    Ok(())
}

fn valid_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn respond_from_git_backend(
    stream: &mut TcpStream,
    project_root: &Path,
    request: &Request,
) -> Result<(), Box<dyn std::error::Error>> {
    let (path, query) = request
        .target
        .split_once('?')
        .unwrap_or((&request.target, ""));
    let path_info = path
        .strip_prefix("/git")
        .filter(|path| path.starts_with('/'))
        .ok_or("request is outside the fixed Git prefix")?;
    let mut child = Command::new("/usr/lib/git-core/git-http-backend")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_PROJECT_ROOT", project_root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", path_info)
        .env("QUERY_STRING", query)
        .env("REQUEST_METHOD", &request.method)
        .env(
            "CONTENT_TYPE",
            header(&request.headers, "content-type").unwrap_or(""),
        )
        .env("CONTENT_LENGTH", request.body.len().to_string())
        .env("REMOTE_USER", "x-access-token")
        .env("REMOTE_ADDR", "127.0.0.1")
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("SERVER_NAME", "127.0.0.1")
        .env("SERVER_PORT", "8787")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("missing Git backend stdin")?
        .write_all(&request.body)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err("Git HTTP backend failed".into());
    }
    let separator = output
        .stdout
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or("Git HTTP backend emitted no header separator")?;
    let headers = std::str::from_utf8(&output.stdout[..separator])?;
    let body = &output.stdout[separator + 4..];
    let mut status = "200 OK".to_owned();
    let mut forwarded = Vec::new();
    for line in headers.lines() {
        let (name, value) = line.split_once(':').ok_or("malformed Git backend header")?;
        if name.eq_ignore_ascii_case("status") {
            status = value.trim().to_owned();
        } else if !name.eq_ignore_ascii_case("content-length")
            && !name.eq_ignore_ascii_case("connection")
        {
            forwarded.push((name.trim(), value.trim()));
        }
    }
    write!(stream, "HTTP/1.1 {status}\r\n")?;
    for (name, value) in forwarded {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(
        stream,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn append_log(path: &Path, line: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}
