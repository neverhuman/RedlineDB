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
    let state_file = args.next().ok_or("missing state file")?;
    let behavior_file = args.next().ok_or("missing behavior file")?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    fs::write(&state_file, "")?;
    fs::write(&behavior_file, "ok\n")?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    fs::write(&address_file, format!("http://{address}\n"))?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle(
                    stream,
                    Path::new(&log_file),
                    Path::new(&state_file),
                    Path::new(&behavior_file),
                ) {
                    eprintln!("fake forge request error: {error}");
                }
            }
            Err(error) => eprintln!("fake forge accept error: {error}"),
        }
    }
    Ok(())
}

fn handle(
    mut stream: TcpStream,
    log_path: &Path,
    state_path: &Path,
    behavior_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts.next().unwrap_or("");
    let path = request_parts.next().unwrap_or("");
    let auth_present = text.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("authorization: bearer ")
    });
    let body = header_end
        .and_then(|end| request.get(end..end.saturating_add(content_length)))
        .map(|bytes| String::from_utf8_lossy(bytes).replace(['\r', '\n'], " "))
        .unwrap_or_default();
    let behavior = fs::read_to_string(behavior_path)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    writeln!(
        log,
        "{request_line}\tauth={auth_present}\tbehavior={behavior}\tbody={body}"
    )?;

    let proof_post = method == "POST"
        && path.ends_with("/check-runs")
        && body.contains("\"name\":\"jankurai/proof\"");
    let required_post =
        method == "POST" && path.ends_with("/check-runs") && body.contains("/required\"");
    let status_post = method == "POST" && path.contains("/statuses/");
    if (behavior == "proof-post-fail" && proof_post)
        || (behavior == "required-post-fail" && required_post)
        || (behavior == "status-post-fail" && status_post)
    {
        return respond(&mut stream, 500, "fixture failure", "text/plain");
    }

    if method == "GET" && path == "/api/v1/repos?host=jeryu" {
        let valid_control = r#"{"id":{"host":"jeryu","owner":"veox","name":"jain-split-ops"},"default_branch":"main","clone_http_url":"/git/veox/jain-split-ops.git"}"#;
        let valid_product = r#"{"id":{"host":"jeryu","owner":"jeryu","name":"jain-report"},"default_branch":"main","clone_http_url":"/git/jeryu/jain-report.git"}"#;
        let control = match behavior.as_str() {
            "authority-wrong-owner" => r#"{"id":{"host":"jeryu","owner":"attacker","name":"jain-split-ops"},"default_branch":"main","clone_http_url":"/git/attacker/jain-split-ops.git"}"#,
            "authority-wrong-default" => r#"{"id":{"host":"jeryu","owner":"veox","name":"jain-split-ops"},"default_branch":"trunk","clone_http_url":"/git/veox/jain-split-ops.git"}"#,
            "authority-wrong-clone" => r#"{"id":{"host":"jeryu","owner":"veox","name":"jain-split-ops"},"default_branch":"main","clone_http_url":"/git/veox/wrong.git"}"#,
            _ => valid_control,
        };
        let mut rows = Vec::new();
        if behavior != "authority-missing" {
            rows.push(control);
        }
        if behavior == "authority-duplicate" {
            rows.push(control);
        }
        rows.push(valid_product);
        let response = format!("{{\"repositories\":[{}]}}", rows.join(","));
        return respond(&mut stream, 200, &response, "application/json");
    }

    if method == "POST" && path.ends_with("/check-runs") {
        let mut state = OpenOptions::new()
            .create(true)
            .append(true)
            .open(state_path)?;
        writeln!(state, "check\t{body}")?;
    }
    if status_post {
        let mut state = OpenOptions::new()
            .create(true)
            .append(true)
            .open(state_path)?;
        writeln!(state, "status\t{body}")?;
    }
    if method == "GET" && path.contains("/commits/") && path.ends_with("/check-runs") {
        let mut runs = Vec::new();
        if behavior != "readback-missing" {
            for line in fs::read_to_string(state_path)?.lines() {
                if let Some(run) = line.strip_prefix("check\t") {
                    runs.push(run.to_owned());
                }
            }
        }
        if behavior == "readback-mismatch" {
            if let Some(sha) = path
                .split("/commits/")
                .nth(1)
                .and_then(|tail| tail.split('/').next())
            {
                let expected = format!("\"head_sha\":\"{sha}\"");
                let forged = format!("\"head_sha\":\"{}\"", "0".repeat(40));
                runs = runs
                    .into_iter()
                    .map(|run| run.replace(&expected, &forged))
                    .collect();
            }
        }
        if behavior == "required-readback-missing"
            && runs.iter().any(|run| run.contains("/required\""))
        {
            runs.clear();
        }
        let response = format!(
            "{{\"total_count\":{},\"check_runs\":[{}]}}",
            runs.len(),
            runs.join(",")
        );
        return respond(&mut stream, 200, &response, "application/json");
    }
    if method == "GET" && path.contains("/commits/") && path.ends_with("/status") {
        let sha = path
            .split("/commits/")
            .nth(1)
            .and_then(|tail| tail.split('/').next())
            .ok_or("status readback has no commit")?;
        let mut statuses = Vec::new();
        if behavior != "status-readback-missing" {
            for line in fs::read_to_string(state_path)?.lines() {
                if let Some(status) = line.strip_prefix("status\t") {
                    statuses.push(status.to_owned());
                }
            }
        }
        if behavior == "readback-mismatch" {
            statuses = statuses
                .into_iter()
                .map(|status| status.replace("/required\"", "/wrong\""))
                .collect();
        }
        let response = format!(
            "{{\"sha\":\"{sha}\",\"total_count\":{},\"statuses\":[{}]}}",
            statuses.len(),
            statuses.join(",")
        );
        return respond(&mut stream, 200, &response, "application/json");
    }
    respond(&mut stream, 200, "{}", "application/json")
}

fn respond(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
    content_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reason = if status == 200 {
        "OK"
    } else {
        "Internal Server Error"
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    Ok(())
}
