use serde_json::{json, Value as JsonValue};
use std::{
    ffi::{CString, OsStr},
    fmt,
    fs::File,
    io::{self, Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpStream},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Component, Path},
    sync::atomic::{compiler_fence, Ordering},
    time::{Duration, Instant},
};

const JERYU_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8787));
const JERYU_HOST: &str = "127.0.0.1:8787";
const MAX_TOKEN_BYTES: u64 = 4096;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const MAX_WIRE_BYTES: usize = MAX_HEADER_BYTES + MAX_BODY_BYTES + 1024 * 1024;
const MAX_HEADER_COUNT: usize = 128;
const MAX_LINE_BYTES: usize = 4 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub struct JeryuError(String);

impl JeryuError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for JeryuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for JeryuError {}

impl From<io::Error> for JeryuError {
    fn from(error: io::Error) -> Self {
        Self::new(format!("local Jeryu transport failed: {error}"))
    }
}

type Result<T> = std::result::Result<T, JeryuError>;

struct SecretBytes(Vec<u8>);

impl SecretBytes {
    fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            // SAFETY: `byte` is a valid, exclusively borrowed byte in this vector.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Post,
    Put,
    Patch,
}

impl Method {
    fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JeryuRequest {
    method: Method,
    path: String,
    body: Option<String>,
}

impl JeryuRequest {
    fn new(method: Method, path: String, body: Option<String>) -> Result<Self> {
        validate_request_path(&path)?;
        if let Some(body) = &body {
            if body.len() > MAX_BODY_BYTES {
                return Err(JeryuError::new("local Jeryu request body is oversized"));
            }
        }
        Ok(Self { method, path, body })
    }

    pub fn method(&self) -> &'static str {
        self.method.as_str()
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub fn repo_list() -> Result<Self> {
        Self::new(Method::Get, "/api/v1/repos?host=jeryu".to_owned(), None)
    }

    pub fn pr_list(repo: &str, state: &str) -> Result<Self> {
        validate_repo_slug(repo)?;
        if !matches!(state, "open" | "closed" | "all") {
            return Err(JeryuError::new("PR state must be open, closed, or all"));
        }
        Self::new(
            Method::Get,
            format!("/repos/{repo}/pulls?state={state}"),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pr_open(
        repo: &str,
        title: &str,
        head: &str,
        expected_head: &str,
        base: &str,
        body: &str,
        draft: bool,
        actor: &str,
    ) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_ref_name(head, "PR head")?;
        validate_sha(expected_head)?;
        validate_ref_name(base, "PR base")?;
        validate_actor(actor)?;
        Self::new(
            Method::Post,
            format!("/repos/{repo}/pulls"),
            Some(
                json!({
                    "title": title,
                    "head": head,
                    "head_sha": expected_head,
                    "base": base,
                    "body": body,
                    "draft": draft,
                    "actor": actor,
                })
                .to_string(),
            ),
        )
    }

    pub fn checks(repo: &str, sha: &str) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_sha(sha)?;
        Self::new(
            Method::Get,
            format!("/repos/{repo}/commits/{sha}/check-runs"),
            None,
        )
    }

    pub fn pr_update(repo: &str, number: u64, body: JsonValue) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_number(number)?;
        Self::new(
            Method::Patch,
            format!("/repos/{repo}/pulls/{number}"),
            Some(body.to_string()),
        )
    }

    pub fn pr_readback(repo: &str, number: u64) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_number(number)?;
        Self::new(
            Method::Get,
            format!("/api/v1/repos/{}/pulls/{number}", encoded_repo(repo)),
            None,
        )
    }

    pub fn pr_details(repo: &str, number: u64) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_number(number)?;
        Self::new(Method::Get, format!("/repos/{repo}/pulls/{number}"), None)
    }

    pub fn pr_approval(
        repo: &str,
        number: u64,
        expected_head: &str,
        body: Option<&str>,
    ) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_number(number)?;
        validate_sha(expected_head)?;
        Self::new(
            Method::Post,
            format!(
                "/api/v1/repos/{}/pulls/{number}/reviews",
                encoded_repo(repo)
            ),
            Some(
                json!({
                    "verdict": "approve",
                    "expected_head_sha": expected_head,
                    "body_markdown": body,
                    "thread_comments": [],
                    "evidence": JsonValue::Null,
                })
                .to_string(),
            ),
        )
    }

    pub fn pr_merge(repo: &str, number: u64, expected_head: &str) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_number(number)?;
        validate_sha(expected_head)?;
        Self::new(
            Method::Put,
            format!("/repos/{repo}/pulls/{number}/merge"),
            Some(json!({"sha": expected_head, "merge_method": "merge"}).to_string()),
        )
    }

    pub fn protection(repo: &str, branch: &str, body: Option<JsonValue>) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_ref_name(branch, "protected branch")?;
        Self::new(
            if body.is_some() {
                Method::Put
            } else {
                Method::Get
            },
            format!("/repos/{repo}/branches/{branch}/protection"),
            body.map(|value| value.to_string()),
        )
    }

    pub fn check_run(repo: &str, body: JsonValue) -> Result<Self> {
        validate_repo_slug(repo)?;
        Self::new(
            Method::Post,
            format!("/repos/{repo}/check-runs"),
            Some(body.to_string()),
        )
    }

    pub fn commit_status(repo: &str, sha: &str, body: JsonValue) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_sha(sha)?;
        Self::new(
            Method::Post,
            format!("/repos/{repo}/statuses/{sha}"),
            Some(body.to_string()),
        )
    }

    pub fn commit_status_readback(repo: &str, sha: &str) -> Result<Self> {
        validate_repo_slug(repo)?;
        validate_sha(sha)?;
        Self::new(
            Method::Get,
            format!("/repos/{repo}/commits/{sha}/status"),
            None,
        )
    }
}

pub struct JeryuClient {
    token: SecretBytes,
    address: SocketAddr,
}

pub struct HostCiPublication {
    pub repo: String,
    pub head_sha: String,
    pub required_check: String,
    pub conclusion: String,
    pub proof_summary: String,
    pub proof_receipt_sha256: String,
    pub proof_attempt_id: String,
    pub status_description: String,
}

impl HostCiPublication {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.conclusion.as_str(), "success" | "failure") {
            return Err(JeryuError::new(
                "host-CI conclusion must be success or failure",
            ));
        }
        validate_repo_slug(&self.repo)?;
        validate_sha(&self.head_sha)?;
        validate_digest(&self.proof_receipt_sha256)?;
        validate_publication_text(&self.required_check, "required check")?;
        validate_publication_text(&self.proof_attempt_id, "proof attempt ID")?;
        validate_publication_text(&self.proof_summary, "proof summary")?;
        let receipt = format!("receipt_sha256={}", self.proof_receipt_sha256);
        let attempt = format!("attempt_id={}", self.proof_attempt_id);
        if self
            .proof_summary
            .split_ascii_whitespace()
            .filter(|value| *value == receipt)
            .count()
            != 1
            || self
                .proof_summary
                .split_ascii_whitespace()
                .filter(|value| *value == attempt)
                .count()
                != 1
        {
            return Err(JeryuError::new(
                "host-CI proof summary must contain the exact receipt and attempt markers once",
            ));
        }
        validate_publication_text(&self.status_description, "status description")
    }
}

#[derive(Debug)]
pub struct PublishFailure {
    publication_started: bool,
    error: JeryuError,
}

impl PublishFailure {
    pub fn publication_started(&self) -> bool {
        self.publication_started
    }
}

impl fmt::Display for PublishFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for PublishFailure {}

impl JeryuClient {
    pub fn from_token_file(path: &Path) -> Result<Self> {
        let token = read_token(path, unsafe { libc::geteuid() })?;
        Ok(Self {
            token,
            address: JERYU_ADDR,
        })
    }

    pub fn execute(&self, request: &JeryuRequest) -> Result<JsonValue> {
        validate_request_path(&request.path)?;
        let mut stream = TcpStream::connect_timeout(&self.address, IO_TIMEOUT)?;
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;

        let mut wire = SecretBytes(Vec::with_capacity(
            256 + self.token.0.len() + request.body.as_ref().map_or(0, String::len),
        ));
        write!(
            wire.0,
            "{} {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nAuthorization: Bearer ",
            request.method.as_str(),
            request.path,
            JERYU_HOST
        )
        .map_err(|error| JeryuError::new(format!("cannot compose Jeryu request: {error}")))?;
        wire.0.extend_from_slice(self.token.as_slice());
        wire.0.extend_from_slice(b"\r\nConnection: close\r\n");
        if let Some(body) = &request.body {
            write!(
                wire.0,
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            )
            .map_err(|error| JeryuError::new(format!("cannot compose Jeryu request: {error}")))?;
        }
        wire.0.extend_from_slice(b"\r\n");
        if let Some(body) = &request.body {
            wire.0.extend_from_slice(body.as_bytes());
        }
        stream.write_all(&wire.0)?;
        stream.shutdown(Shutdown::Write)?;
        drop(wire);

        let response = read_response(&mut stream, Instant::now() + IO_TIMEOUT)?;
        if !(200..300).contains(&response.status) {
            let message = redact_and_sanitize(&response.body, self.token.as_slice());
            return Err(JeryuError::new(format!(
                "local Jeryu returned HTTP {}: {}",
                response.status, message
            )));
        }
        if contains_bytes(&response.body, self.token.as_slice()) {
            return Err(JeryuError::new(
                "local Jeryu response reflected the authentication credential",
            ));
        }
        if response.body.is_empty() {
            Ok(JsonValue::Null)
        } else {
            serde_json::from_slice(&response.body).map_err(|error| {
                JeryuError::new(format!("local Jeryu returned invalid JSON: {error}"))
            })
        }
    }

    pub fn publish_host_ci(
        &self,
        publication: &HostCiPublication,
    ) -> std::result::Result<(), PublishFailure> {
        publication.validate().map_err(PublishFailure::before)?;

        let proof = JeryuRequest::check_run(
            &publication.repo,
            json!({
                "name": "jankurai/proof",
                "head_sha": publication.head_sha,
                "status": "completed",
                "conclusion": publication.conclusion,
                "output": {
                    "title": "Root-sealed exact-SHA Jankurai proof",
                    "summary": publication.proof_summary,
                }
            }),
        )
        .map_err(PublishFailure::before)?;
        self.execute(&proof).map_err(PublishFailure::after)?;

        let readback = JeryuRequest::checks(&publication.repo, &publication.head_sha)
            .map_err(PublishFailure::after)?;
        let response = self.execute(&readback).map_err(PublishFailure::after)?;
        validate_proof_readback(&response, publication).map_err(PublishFailure::after)?;

        let required = JeryuRequest::check_run(
            &publication.repo,
            json!({
                "name": publication.required_check,
                "head_sha": publication.head_sha,
                "status": "completed",
                "conclusion": publication.conclusion,
            }),
        )
        .map_err(PublishFailure::after)?;
        self.execute(&required).map_err(PublishFailure::after)?;

        let required_readback = JeryuRequest::checks(&publication.repo, &publication.head_sha)
            .map_err(PublishFailure::after)?;
        let response = self
            .execute(&required_readback)
            .map_err(PublishFailure::after)?;
        validate_required_readback(&response, publication).map_err(PublishFailure::after)?;

        let status = JeryuRequest::commit_status(
            &publication.repo,
            &publication.head_sha,
            json!({
                "state": if publication.conclusion == "success" {
                    "success"
                } else {
                    "failure"
                },
                "context": publication.required_check,
                "description": publication.status_description,
            }),
        )
        .map_err(PublishFailure::after)?;
        self.execute(&status).map_err(PublishFailure::after)?;

        let status_readback =
            JeryuRequest::commit_status_readback(&publication.repo, &publication.head_sha)
                .map_err(PublishFailure::after)?;
        let response = self
            .execute(&status_readback)
            .map_err(PublishFailure::after)?;
        validate_status_readback(&response, publication).map_err(PublishFailure::after)?;
        Ok(())
    }

    #[cfg(test)]
    fn for_test(token: &[u8], address: SocketAddr) -> Self {
        Self {
            token: SecretBytes(token.to_vec()),
            address,
        }
    }
}

impl PublishFailure {
    fn before(error: JeryuError) -> Self {
        Self {
            publication_started: false,
            error,
        }
    }

    fn after(error: JeryuError) -> Self {
        Self {
            publication_started: true,
            error,
        }
    }
}

fn validate_publication_text(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
    {
        return Err(JeryuError::new(format!("host-CI {label} is unsafe")));
    }
    Ok(())
}

fn validate_proof_readback(response: &JsonValue, publication: &HostCiPublication) -> Result<()> {
    let runs = response
        .get("check_runs")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| JeryuError::new("Jeryu proof readback has no check_runs array"))?;
    let matches = runs
        .iter()
        .filter(|run| {
            run.get("name").and_then(JsonValue::as_str) == Some("jankurai/proof")
                && run.get("head_sha").and_then(JsonValue::as_str)
                    == Some(publication.head_sha.as_str())
                && run.get("status").and_then(JsonValue::as_str) == Some("completed")
                && run.get("conclusion").and_then(JsonValue::as_str)
                    == Some(publication.conclusion.as_str())
                && run
                    .get("output")
                    .and_then(|output| output.get("summary"))
                    .and_then(JsonValue::as_str)
                    == Some(publication.proof_summary.as_str())
        })
        .count();
    if matches == 1 {
        Ok(())
    } else {
        Err(JeryuError::new(
            "Jeryu proof readback does not exactly match the published proof",
        ))
    }
}

fn validate_required_readback(response: &JsonValue, publication: &HostCiPublication) -> Result<()> {
    let runs = response
        .get("check_runs")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| JeryuError::new("Jeryu required-check readback has no check_runs array"))?;
    let matches = runs
        .iter()
        .filter(|run| {
            run.get("name").and_then(JsonValue::as_str) == Some(publication.required_check.as_str())
                && run.get("head_sha").and_then(JsonValue::as_str)
                    == Some(publication.head_sha.as_str())
                && run.get("status").and_then(JsonValue::as_str) == Some("completed")
                && run.get("conclusion").and_then(JsonValue::as_str)
                    == Some(publication.conclusion.as_str())
        })
        .count();
    if matches == 1 {
        Ok(())
    } else {
        Err(JeryuError::new(
            "Jeryu required-check readback does not exactly match the publication",
        ))
    }
}

fn validate_status_readback(response: &JsonValue, publication: &HostCiPublication) -> Result<()> {
    if response.get("sha").and_then(JsonValue::as_str) != Some(publication.head_sha.as_str()) {
        return Err(JeryuError::new(
            "Jeryu commit-status readback names the wrong commit",
        ));
    }
    let expected_state = if publication.conclusion == "success" {
        "success"
    } else {
        "failure"
    };
    let statuses = response
        .get("statuses")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| JeryuError::new("Jeryu commit-status readback has no statuses array"))?;
    let matches = statuses
        .iter()
        .filter(|status| {
            status.get("context").and_then(JsonValue::as_str)
                == Some(publication.required_check.as_str())
                && status.get("state").and_then(JsonValue::as_str) == Some(expected_state)
                && status.get("description").and_then(JsonValue::as_str)
                    == Some(publication.status_description.as_str())
        })
        .count();
    if matches == 1 {
        Ok(())
    } else {
        Err(JeryuError::new(
            "Jeryu commit-status readback does not exactly match the publication",
        ))
    }
}

fn validate_repo_slug(repo: &str) -> Result<()> {
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if parts.next().is_some() || !safe_component(owner) || !safe_component(name) {
        return Err(JeryuError::new("repository must be a safe owner/name slug"));
    }
    Ok(())
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.starts_with('.')
        && !value.ends_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn validate_ref_name(value: &str, label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 255
        || value.starts_with('.')
        || value.ends_with('.')
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains("..")
        || value.contains("//")
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        return Err(JeryuError::new(format!("{label} is unsafe")));
    }
    Ok(())
}

fn validate_actor(actor: &str) -> Result<()> {
    if !safe_component(actor) {
        return Err(JeryuError::new("PR actor is unsafe"));
    }
    Ok(())
}

fn validate_sha(sha: &str) -> Result<()> {
    if sha.len() != 40 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(JeryuError::new(
            "commit SHA must contain 40 hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_digest(digest: &str) -> Result<()> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(JeryuError::new(
            "SHA-256 digest must contain 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn validate_number(number: u64) -> Result<()> {
    if number == 0 {
        Err(JeryuError::new("PR number must be positive"))
    } else {
        Ok(())
    }
}

fn encoded_repo(repo: &str) -> String {
    repo.replace('/', "%2F")
}

fn validate_request_path(path: &str) -> Result<()> {
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.len() > 4096
        || path.contains('\\')
        || path.contains('#')
        || path
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ' || byte >= 0x7f)
        || path.split(['/', '?']).any(|component| component == "..")
    {
        return Err(JeryuError::new("local Jeryu request path is unsafe"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
    nlink: u64,
}

impl Identity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            uid: metadata.uid(),
            mode: metadata.mode(),
            nlink: metadata.nlink(),
        }
    }
}

struct OpenedToken {
    file: File,
    identity: Identity,
    component_ids: Vec<(u64, u64)>,
}

fn read_token(path: &Path, expected_uid: u32) -> Result<SecretBytes> {
    read_token_with_hook(path, expected_uid, || {})
}

fn read_token_with_hook<F>(path: &Path, expected_uid: u32, hook: F) -> Result<SecretBytes>
where
    F: FnOnce(),
{
    let mut opened = open_token(path, expected_uid)?;
    let mut bytes = SecretBytes(Vec::new());
    (&mut opened.file)
        .take(MAX_TOKEN_BYTES + 1)
        .read_to_end(&mut bytes.0)?;
    if bytes.0.len() as u64 > MAX_TOKEN_BYTES {
        return Err(JeryuError::new("Jeryu token file is oversized"));
    }
    let after_read = Identity::from_metadata(&opened.file.metadata()?);
    if after_read != opened.identity {
        return Err(JeryuError::new(
            "Jeryu token descriptor changed while reading",
        ));
    }

    hook();
    let reopened = open_token(path, expected_uid)?;
    if reopened.identity != opened.identity || reopened.component_ids != opened.component_ids {
        return Err(JeryuError::new("Jeryu token path changed while reading"));
    }
    validate_token_bytes(bytes.as_slice())?;
    Ok(bytes)
}

fn open_token(path: &Path, expected_uid: u32) -> Result<OpenedToken> {
    if !path.is_absolute() {
        return Err(JeryuError::new("Jeryu token path must be absolute"));
    }
    let mut normal = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(value) => normal.push(value),
            _ => {
                return Err(JeryuError::new(
                    "Jeryu token path contains an unsafe component",
                ))
            }
        }
    }
    let final_name = normal
        .pop()
        .ok_or_else(|| JeryuError::new("Jeryu token path names no file"))?;
    let root = open_at(
        libc::AT_FDCWD,
        OsStr::new("/"),
        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
    )?;
    let mut directory = File::from(root);
    let mut component_ids = Vec::new();
    for component in normal {
        let next = open_at(
            directory.as_raw_fd(),
            component,
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | libc::O_CLOEXEC,
        )?;
        let next = File::from(next);
        let metadata = next.metadata()?;
        component_ids.push((metadata.dev(), metadata.ino()));
        directory = next;
    }
    let descriptor = open_at(
        directory.as_raw_fd(),
        final_name,
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
    )?;
    let file = File::from(descriptor);
    let metadata = file.metadata()?;
    let identity = Identity::from_metadata(&metadata);
    if !metadata.file_type().is_file() {
        return Err(JeryuError::new("Jeryu token must be a regular file"));
    }
    if identity.uid != expected_uid {
        return Err(JeryuError::new(
            "Jeryu token must be owned by the current user",
        ));
    }
    if identity.mode & 0o7777 != 0o600 {
        return Err(JeryuError::new("Jeryu token mode must be exactly 0600"));
    }
    if identity.nlink != 1 {
        return Err(JeryuError::new("Jeryu token must have exactly one link"));
    }
    Ok(OpenedToken {
        file,
        identity,
        component_ids,
    })
}

fn open_at(directory: i32, name: &OsStr, flags: i32) -> Result<OwnedFd> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| JeryuError::new("Jeryu token path contains a NUL byte"))?;
    // SAFETY: `name` is NUL-terminated, `directory` is either AT_FDCWD or a live
    // directory descriptor, and ownership of a successful descriptor is transferred.
    let descriptor = unsafe { libc::openat(directory, name.as_ptr(), flags, 0) };
    if descriptor < 0 {
        return Err(JeryuError::new(format!(
            "cannot securely open Jeryu token path: {}",
            io::Error::last_os_error()
        )));
    }
    // SAFETY: `openat` returned a new owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(descriptor) })
}

fn validate_token_bytes(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 16 {
        return Err(JeryuError::new("Jeryu token is empty or too short"));
    }
    if bytes.iter().any(|byte| {
        !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'~' | b'+' | b'/' | b'-')
    }) {
        return Err(JeryuError::new(
            "Jeryu token contains whitespace, control, or unsupported bytes",
        ));
    }
    Ok(())
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn read_response(stream: &mut TcpStream, deadline: Instant) -> Result<HttpResponse> {
    let mut wire = Vec::new();
    let header_end = loop {
        if let Some(index) = find_bytes(&wire, b"\r\n\r\n") {
            break index + 4;
        }
        if wire.len() > MAX_HEADER_BYTES {
            return Err(JeryuError::new(
                "local Jeryu response headers are oversized",
            ));
        }
        if !read_more(stream, &mut wire, deadline)? {
            return Err(JeryuError::new(
                "local Jeryu response headers are truncated",
            ));
        }
    };
    if header_end > MAX_HEADER_BYTES {
        return Err(JeryuError::new(
            "local Jeryu response headers are oversized",
        ));
    }
    let head = parse_response_head(&wire[..header_end])?;
    let mut body_wire = wire.split_off(header_end);
    let body = if head.status == 204 {
        if !body_wire.is_empty() {
            return Err(JeryuError::new("HTTP 204 response carried a body"));
        }
        Vec::new()
    } else if let Some(length) = head.content_length {
        if length > MAX_BODY_BYTES {
            return Err(JeryuError::new("local Jeryu response body is oversized"));
        }
        while body_wire.len() < length {
            if !read_more(stream, &mut body_wire, deadline)? {
                return Err(JeryuError::new("local Jeryu response body is truncated"));
            }
        }
        if body_wire.len() != length {
            return Err(JeryuError::new("local Jeryu response has trailing bytes"));
        }
        body_wire
    } else if head.chunked {
        read_chunked_body(stream, body_wire, deadline)?
    } else if head.close_delimited {
        if body_wire.len() > MAX_BODY_BYTES {
            return Err(JeryuError::new("local Jeryu response body is oversized"));
        }
        while read_more(stream, &mut body_wire, deadline)? {
            if body_wire.len() > MAX_BODY_BYTES {
                return Err(JeryuError::new("local Jeryu response body is oversized"));
            }
        }
        body_wire
    } else {
        return Err(JeryuError::new(
            "HTTP/1.1 response has no explicit body framing",
        ));
    };
    Ok(HttpResponse {
        status: head.status,
        body,
    })
}

fn read_more(stream: &mut TcpStream, bytes: &mut Vec<u8>, deadline: Instant) -> Result<bool> {
    if bytes.len() >= MAX_WIRE_BYTES {
        return Err(JeryuError::new("local Jeryu response is oversized"));
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| JeryuError::new("local Jeryu response deadline exceeded"))?;
    stream.set_read_timeout(Some(remaining.min(IO_TIMEOUT)))?;
    let mut buffer = [0_u8; 8192];
    let count = stream.read(&mut buffer).map_err(|error| {
        if matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ) {
            JeryuError::new("local Jeryu response deadline exceeded")
        } else {
            error.into()
        }
    })?;
    if count == 0 {
        return Ok(false);
    }
    bytes.extend_from_slice(&buffer[..count]);
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(JeryuError::new("local Jeryu response is oversized"));
    }
    Ok(true)
}

struct ResponseHead {
    status: u16,
    content_length: Option<usize>,
    chunked: bool,
    close_delimited: bool,
}

fn parse_response_head(bytes: &[u8]) -> Result<ResponseHead> {
    if !bytes.ends_with(b"\r\n\r\n") || !valid_crlf(bytes) {
        return Err(JeryuError::new(
            "local Jeryu response uses malformed line endings",
        ));
    }
    let text = std::str::from_utf8(&bytes[..bytes.len() - 4])
        .map_err(|_| JeryuError::new("local Jeryu response headers are not UTF-8"))?;
    let mut lines = text.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| JeryuError::new("local Jeryu response has no status line"))?;
    if status_line.len() > MAX_LINE_BYTES {
        return Err(JeryuError::new(
            "local Jeryu response status line is oversized",
        ));
    }
    let mut status_parts = status_line.splitn(3, ' ');
    let version = status_parts.next().unwrap_or_default();
    let status_text = status_parts.next().unwrap_or_default();
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || status_text.len() != 3
        || !status_text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(JeryuError::new(
            "local Jeryu response status line is malformed",
        ));
    }
    let status = status_text
        .parse::<u16>()
        .map_err(|_| JeryuError::new("local Jeryu response status is malformed"))?;
    if !(100..600).contains(&status) {
        return Err(JeryuError::new(
            "local Jeryu response status is out of range",
        ));
    }

    let mut content_length = None;
    let mut transfer_encoding = None;
    let mut connection = None;
    for (index, line) in lines.enumerate() {
        if index >= MAX_HEADER_COUNT {
            return Err(JeryuError::new("local Jeryu response has too many headers"));
        }
        if line.len() > MAX_LINE_BYTES {
            return Err(JeryuError::new(
                "local Jeryu response header line is oversized",
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| JeryuError::new("local Jeryu response header is malformed"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value
                .bytes()
                .any(|byte| byte.is_ascii_control() && byte != b'\t')
        {
            return Err(JeryuError::new("local Jeryu response header is malformed"));
        }
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some()
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(JeryuError::new(
                    "local Jeryu response has duplicate or invalid content-length",
                ));
            }
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| JeryuError::new("local Jeryu content-length is oversized"))?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            if transfer_encoding.is_some() {
                return Err(JeryuError::new(
                    "local Jeryu response has duplicate transfer-encoding",
                ));
            }
            transfer_encoding = Some(value.to_ascii_lowercase());
        } else if name.eq_ignore_ascii_case("connection") {
            if connection.is_some() {
                return Err(JeryuError::new(
                    "local Jeryu response has duplicate connection headers",
                ));
            }
            connection = Some(value.to_ascii_lowercase());
        } else if name.eq_ignore_ascii_case("content-encoding")
            && !value.eq_ignore_ascii_case("identity")
        {
            return Err(JeryuError::new(
                "local Jeryu response uses an unsupported content encoding",
            ));
        }
    }
    if content_length.is_some() && transfer_encoding.is_some() {
        return Err(JeryuError::new(
            "local Jeryu response mixes content-length and transfer-encoding",
        ));
    }
    let chunked = match transfer_encoding.as_deref() {
        None => false,
        Some("chunked") => true,
        Some(_) => {
            return Err(JeryuError::new(
                "local Jeryu response uses an unsupported transfer encoding",
            ))
        }
    };
    if status == 204 && (chunked || content_length.is_some_and(|length| length != 0)) {
        return Err(JeryuError::new("HTTP 204 response declares a body"));
    }
    let explicit_close = connection.as_deref().is_some_and(|value| {
        value
            .split(',')
            .map(str::trim)
            .any(|token| token.eq_ignore_ascii_case("close"))
    });
    let close_delimited = content_length.is_none()
        && !chunked
        && status != 204
        && (version == "HTTP/1.0" || explicit_close);
    Ok(ResponseHead {
        status,
        content_length,
        chunked,
        close_delimited,
    })
}

fn valid_crlf(bytes: &[u8]) -> bool {
    bytes.iter().enumerate().all(|(index, byte)| match byte {
        b'\n' => index > 0 && bytes[index - 1] == b'\r',
        b'\r' => bytes.get(index + 1) == Some(&b'\n'),
        _ => true,
    })
}

fn read_chunked_body(
    stream: &mut TcpStream,
    mut bytes: Vec<u8>,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let mut cursor = 0;
    let mut decoded = Vec::new();
    loop {
        let line_end = read_line_end(stream, &mut bytes, cursor, deadline, "chunk size")?;
        let line = &bytes[cursor..line_end];
        if line.is_empty() || line.len() > 16 || !line.iter().all(u8::is_ascii_hexdigit) {
            return Err(JeryuError::new("local Jeryu chunk size is malformed"));
        }
        let size_text = std::str::from_utf8(line)
            .map_err(|_| JeryuError::new("local Jeryu chunk size is malformed"))?;
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| JeryuError::new("local Jeryu chunk size is malformed"))?;
        cursor = line_end + 2;
        if size == 0 {
            for trailer_count in 0..=MAX_HEADER_COUNT {
                let trailer_end = read_line_end(stream, &mut bytes, cursor, deadline, "trailer")?;
                if trailer_end == cursor {
                    if trailer_end + 2 != bytes.len() {
                        return Err(JeryuError::new(
                            "local Jeryu chunked body has trailing bytes",
                        ));
                    }
                    return Ok(decoded);
                }
                if trailer_count == MAX_HEADER_COUNT {
                    return Err(JeryuError::new(
                        "local Jeryu response has too many trailers",
                    ));
                }
                let trailer = &bytes[cursor..trailer_end];
                validate_trailer(trailer)?;
                cursor = trailer_end + 2;
            }
            unreachable!();
        }
        if size > MAX_BODY_BYTES.saturating_sub(decoded.len()) {
            return Err(JeryuError::new("local Jeryu chunked body is oversized"));
        }
        let Some(data_end) = cursor.checked_add(size) else {
            return Err(JeryuError::new("local Jeryu chunk size is oversized"));
        };
        while bytes.len() < data_end + 2 {
            if !read_more(stream, &mut bytes, deadline)? {
                return Err(JeryuError::new("local Jeryu chunked body is truncated"));
            }
        }
        if &bytes[data_end..data_end + 2] != b"\r\n" {
            return Err(JeryuError::new("local Jeryu chunk terminator is malformed"));
        }
        decoded.extend_from_slice(&bytes[cursor..data_end]);
        cursor = data_end + 2;
    }
}

fn read_line_end(
    stream: &mut TcpStream,
    bytes: &mut Vec<u8>,
    start: usize,
    deadline: Instant,
    label: &str,
) -> Result<usize> {
    loop {
        if let Some(relative) = find_bytes(&bytes[start..], b"\r\n") {
            let end = start + relative;
            if end - start > MAX_LINE_BYTES {
                return Err(JeryuError::new(format!(
                    "local Jeryu {label} line is oversized"
                )));
            }
            return Ok(end);
        }
        if bytes.len().saturating_sub(start) > MAX_LINE_BYTES {
            return Err(JeryuError::new(format!(
                "local Jeryu {label} line is oversized"
            )));
        }
        if !read_more(stream, bytes, deadline)? {
            return Err(JeryuError::new(format!(
                "local Jeryu {label} line is truncated"
            )));
        }
    }
}

fn validate_trailer(line: &[u8]) -> Result<()> {
    if line.len() > MAX_LINE_BYTES {
        return Err(JeryuError::new("local Jeryu trailer line is oversized"));
    }
    let text = std::str::from_utf8(line)
        .map_err(|_| JeryuError::new("local Jeryu trailer is malformed"))?;
    let (name, value) = text
        .split_once(':')
        .ok_or_else(|| JeryuError::new("local Jeryu trailer is malformed"))?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() && byte != b'\t')
        || name.eq_ignore_ascii_case("content-length")
        || name.eq_ignore_ascii_case("transfer-encoding")
    {
        return Err(JeryuError::new("local Jeryu trailer is malformed"));
    }
    Ok(())
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn redact_and_sanitize(body: &[u8], secret: &[u8]) -> String {
    let mut redacted = Vec::with_capacity(body.len().min(1024));
    let mut cursor = 0;
    while cursor < body.len() && redacted.len() < 1024 {
        if !secret.is_empty() && body[cursor..].starts_with(secret) {
            redacted.extend_from_slice(b"[REDACTED]");
            cursor += secret.len();
            continue;
        }
        let byte = body[cursor];
        redacted.push(if byte.is_ascii_graphic() || byte == b' ' {
            byte
        } else {
            b'?'
        });
        cursor += 1;
    }
    String::from_utf8_lossy(&redacted).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        net::TcpListener,
        os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt},
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
        thread,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, AtomicOrdering::Relaxed);
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/test-tmp")
                .join(format!("splitctl-jeryu-client-{}-{id}", std::process::id()));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn token(&self, value: &[u8]) -> std::path::PathBuf {
            let path = self.0.join("token");
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(value).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn token_value() -> &'static [u8] {
        b"fixture-token-0123456789"
    }

    #[test]
    fn secure_token_file_is_accepted() {
        let temp = TempDir::new();
        let path = temp.token(token_value());
        assert_eq!(
            read_token(&path, unsafe { libc::geteuid() })
                .unwrap()
                .as_slice(),
            token_value()
        );
        let client = JeryuClient::from_token_file(&path).unwrap();
        drop(client);
        for entry in fs::read_dir("/proc/self/fd").unwrap() {
            let Ok(entry) = entry else { continue };
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            assert_ne!(target, path, "token descriptor remained open after loading");
        }
    }

    #[test]
    fn token_requires_absolute_path_current_owner_and_exact_mode() {
        let temp = TempDir::new();
        let path = temp.token(token_value());
        assert!(read_token(Path::new("relative"), unsafe { libc::geteuid() }).is_err());
        assert!(read_token(&path, unsafe { libc::geteuid() } + 1).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(read_token(&path, unsafe { libc::geteuid() }).is_err());
    }

    #[test]
    fn token_rejects_symlink_components_and_final_symlink() {
        let temp = TempDir::new();
        let real = temp.0.join("real");
        fs::create_dir(&real).unwrap();
        let path = real.join("token");
        fs::write(&path, token_value()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let linked_dir = temp.0.join("linked-dir");
        symlink(&real, &linked_dir).unwrap();
        assert!(read_token(&linked_dir.join("token"), unsafe { libc::geteuid() }).is_err());
        let linked_file = temp.0.join("linked-token");
        symlink(&path, &linked_file).unwrap();
        assert!(read_token(&linked_file, unsafe { libc::geteuid() }).is_err());
    }

    #[test]
    fn token_rejects_hardlinks_special_files_and_oversize() {
        let temp = TempDir::new();
        let path = temp.token(token_value());
        fs::hard_link(&path, temp.0.join("second-link")).unwrap();
        assert!(read_token(&path, unsafe { libc::geteuid() }).is_err());
        assert!(read_token(Path::new("/dev/null"), unsafe { libc::geteuid() }).is_err());

        let fifo = temp.0.join("fifo");
        let fifo_name = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        assert!(read_token(&fifo, unsafe { libc::geteuid() }).is_err());

        let oversized = temp.0.join("oversized");
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&oversized)
            .unwrap();
        file.set_len(MAX_TOKEN_BYTES + 1).unwrap();
        assert!(read_token(&oversized, unsafe { libc::geteuid() }).is_err());
    }

    #[test]
    fn token_rejects_control_bytes_and_path_replacement() {
        let temp = TempDir::new();
        let control = temp.token(b"fixture-token-with-newline\n");
        assert!(read_token(&control, unsafe { libc::geteuid() }).is_err());

        fs::remove_file(&control).unwrap();
        let path = temp.token(token_value());
        let replacement = temp.0.join("replacement");
        fs::write(&replacement, b"replacement-token-012345").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        let path_for_hook = path.clone();
        assert!(read_token_with_hook(&path, unsafe { libc::geteuid() }, || {
            fs::rename(&replacement, &path_for_hook).unwrap();
        })
        .is_err());
    }

    #[test]
    fn token_rejects_parent_component_replacement() {
        let temp = TempDir::new();
        let held = temp.0.join("held");
        let replacement = temp.0.join("replacement-dir");
        fs::create_dir(&held).unwrap();
        fs::create_dir(&replacement).unwrap();
        for directory in [&held, &replacement] {
            let path = directory.join("token");
            fs::write(&path, token_value()).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let old = temp.0.join("old-held");
        assert!(
            read_token_with_hook(&held.join("token"), unsafe { libc::geteuid() }, || {
                fs::rename(&held, &old).unwrap();
                fs::rename(&replacement, &held).unwrap();
            },)
            .is_err()
        );
    }

    fn serve(response_parts: Vec<Vec<u8>>) -> (SocketAddr, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            loop {
                let mut buffer = [0_u8; 4096];
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => request.extend_from_slice(&buffer[..count]),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) =>
                    {
                        break
                    }
                    Err(error) => panic!("server read failed: {error}"),
                }
            }
            for part in response_parts {
                if let Err(error) = stream.write_all(&part) {
                    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
                    break;
                }
                thread::yield_now();
            }
            request
        });
        (address, handle)
    }

    fn execute_response(
        response_parts: Vec<Vec<u8>>,
    ) -> std::result::Result<JsonValue, JeryuError> {
        let (address, server) = serve(response_parts);
        let client = JeryuClient::for_test(token_value(), address);
        let result = client.execute(&JeryuRequest::repo_list().unwrap());
        let request = server.join().unwrap();
        let text = String::from_utf8(request).unwrap();
        assert!(text.starts_with("GET /api/v1/repos?host=jeryu HTTP/1.1\r\n"));
        assert!(text.contains("\r\nHost: 127.0.0.1:8787\r\n"));
        assert!(text.contains("\r\nAuthorization: Bearer fixture-token-0123456789\r\n"));
        result
    }

    fn serve_sequence(responses: Vec<Vec<u8>>) -> (SocketAddr, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let mut request_lines = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream.read_to_end(&mut request).unwrap();
                assert_proc_non_disclosure(token_value());
                let request = String::from_utf8(request).unwrap();
                request_lines.push(request.lines().next().unwrap_or_default().to_owned());
                stream.write_all(&response).unwrap();
            }
            request_lines
        });
        (address, handle)
    }

    fn json_response(status: &str, value: JsonValue) -> Vec<u8> {
        let body = value.to_string();
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn host_ci_publication() -> HostCiPublication {
        HostCiPublication {
            repo: "jeryu/example".to_owned(),
            head_sha: "a".repeat(40),
            required_check: "example/required".to_owned(),
            conclusion: "success".to_owned(),
            proof_summary: format!(
                "receipt_sha256={} attempt_id=attempt-1 exact",
                "b".repeat(64)
            ),
            proof_receipt_sha256: "b".repeat(64),
            proof_attempt_id: "attempt-1".to_owned(),
            status_description: "example/required root-seal=0123456789abcdef".to_owned(),
        }
    }

    #[test]
    fn host_ci_publication_preserves_proof_readback_check_status_order() {
        let publication = host_ci_publication();
        let proof_run = json!({
            "name": "jankurai/proof",
            "head_sha": publication.head_sha,
            "status": "completed",
            "conclusion": publication.conclusion,
            "output": {"summary": publication.proof_summary},
        });
        let responses = vec![
            json_response("201 Created", json!({})),
            json_response("200 OK", json!({"check_runs": [proof_run.clone()]})),
            json_response("201 Created", json!({})),
            json_response(
                "200 OK",
                json!({"check_runs": [
                    proof_run,
                    {
                        "name": publication.required_check,
                        "head_sha": publication.head_sha,
                        "status": "completed",
                        "conclusion": publication.conclusion,
                    }
                ]}),
            ),
            json_response("201 Created", json!({})),
            json_response(
                "200 OK",
                json!({
                    "sha": publication.head_sha,
                    "statuses": [{
                        "context": publication.required_check,
                        "state": "success",
                        "description": publication.status_description,
                    }]
                }),
            ),
        ];
        let (address, server) = serve_sequence(responses);
        let client = JeryuClient::for_test(token_value(), address);
        client.publish_host_ci(&publication).unwrap();
        assert_eq!(
            server.join().unwrap(),
            vec![
                "POST /repos/jeryu/example/check-runs HTTP/1.1",
                "GET /repos/jeryu/example/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs HTTP/1.1",
                "POST /repos/jeryu/example/check-runs HTTP/1.1",
                "GET /repos/jeryu/example/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/check-runs HTTP/1.1",
                "POST /repos/jeryu/example/statuses/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa HTTP/1.1",
                "GET /repos/jeryu/example/commits/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/status HTTP/1.1",
            ]
        );
    }

    #[test]
    fn host_ci_publication_reports_whether_proof_post_started() {
        let publication = host_ci_publication();
        let (address, server) = serve_sequence(vec![json_response(
            "403 Forbidden",
            json!({"error": "denied"}),
        )]);
        let client = JeryuClient::for_test(token_value(), address);
        let failure = client.publish_host_ci(&publication).unwrap_err();
        assert!(failure.publication_started());
        assert_eq!(server.join().unwrap().len(), 1);

        let (address, server) = serve_sequence(vec![
            json_response("201 Created", json!({})),
            json_response("200 OK", json!({"check_runs": []})),
        ]);
        let client = JeryuClient::for_test(token_value(), address);
        let failure = client.publish_host_ci(&publication).unwrap_err();
        assert!(failure.publication_started());
        assert_eq!(server.join().unwrap().len(), 2);
    }

    #[test]
    fn accepts_fragmented_content_length_zero_204_and_close_delimited() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(
            execute_response(response.iter().map(|byte| vec![*byte]).collect()).unwrap(),
            json!({})
        );
        assert_eq!(
            execute_response(vec![
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec()
            ])
            .unwrap(),
            JsonValue::Null
        );
        assert_eq!(
            execute_response(vec![b"HTTP/1.1 204 No Content\r\n\r\n".to_vec()]).unwrap(),
            JsonValue::Null
        );
        assert_eq!(
            execute_response(vec![b"HTTP/1.0 200 OK\r\n\r\n{}".to_vec()]).unwrap(),
            json!({})
        );
        assert_eq!(
            execute_response(vec![
                b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}".to_vec()
            ])
            .unwrap(),
            json!({})
        );
        assert!(execute_response(vec![b"HTTP/1.1 200 OK\r\n\r\n{}".to_vec()]).is_err());
    }

    #[test]
    fn accepts_chunked_body_and_trailers() {
        let parts = vec![
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
            b"1\r\n{\r\n1\r\n}\r\n0\r\nX-Proof: exact\r\n\r\n".to_vec(),
        ];
        assert_eq!(execute_response(parts).unwrap(), json!({}));
    }

    #[test]
    fn rejects_ambiguous_or_unsupported_response_framing() {
        let cases: &[&[u8]] = &[
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{}",
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n{}",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n{}",
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 2\r\n\r\n{}",
            b"HTTP/1.1 204 No Content\r\nContent-Length: 2\r\n\r\n{}",
        ];
        for response in cases {
            assert!(execute_response(vec![response.to_vec()]).is_err());
        }
    }

    #[test]
    fn rejects_malformed_or_truncated_chunked_responses() {
        let cases: &[&[u8]] = &[
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nZ\r\n{}\r\n0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}XX0\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nContent-Length: 1\r\n\r\n",
        ];
        for response in cases {
            assert!(execute_response(vec![response.to_vec()]).is_err());
        }
    }

    #[test]
    fn rejects_truncation_malformed_status_and_oversize() {
        assert!(execute_response(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n{}".to_vec()
        ])
        .is_err());
        assert!(execute_response(vec![b"HTTP/1.1 two OK\r\n\r\n".to_vec()]).is_err());
        let mut oversized = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        oversized.resize(MAX_BODY_BYTES + MAX_HEADER_BYTES + 1, b'x');
        assert!(execute_response(vec![oversized]).is_err());

        let mut too_many_headers = b"HTTP/1.1 200 OK\r\n".to_vec();
        for index in 0..=MAX_HEADER_COUNT {
            too_many_headers.extend_from_slice(format!("X-{index}: value\r\n").as_bytes());
        }
        too_many_headers.extend_from_slice(b"Content-Length: 2\r\n\r\n{}");
        assert!(execute_response(vec![too_many_headers]).is_err());

        let oversized_line = format!(
            "HTTP/1.1 200 OK\r\nX-Long: {}\r\nContent-Length: 2\r\n\r\n{{}}",
            "x".repeat(MAX_LINE_BYTES + 1)
        );
        assert!(execute_response(vec![oversized_line.into_bytes()]).is_err());
    }

    #[test]
    fn rejects_path_injection_before_connecting() {
        assert!(JeryuRequest::pr_list("owner/repo\r\nInjected: yes", "open").is_err());
        assert!(JeryuRequest::pr_list("owner/repo", "open&admin=true").is_err());
        assert!(JeryuRequest::new(Method::Get, "//wrong-origin/path".to_owned(), None).is_err());
        assert!(JeryuRequest::new(Method::Get, "/ok\r\nX: injected".to_owned(), None).is_err());
    }

    #[test]
    fn non_success_errors_redact_reflected_secret() {
        let body = b"denied fixture-token-0123456789 details";
        let mut response = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        let error = execute_response(vec![response]).unwrap_err().to_string();
        assert!(error.contains("[REDACTED]"));
        assert!(!error.contains("fixture-token-0123456789"));
        let receipt = serde_json::to_string(&json!({"status": "fail", "error": error})).unwrap();
        let log = format!("publisher failed: {receipt}");
        assert!(!receipt.contains("fixture-token-0123456789"));
        assert!(!log.contains("fixture-token-0123456789"));

        let body = br#"{"token":"fixture-token-0123456789"}"#;
        let mut response =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes();
        response.extend_from_slice(body);
        let error = execute_response(vec![response]).unwrap_err().to_string();
        assert!(error.contains("reflected the authentication credential"));
        assert!(!error.contains("fixture-token-0123456789"));
    }

    #[test]
    fn client_debug_and_proc_metadata_do_not_disclose_secret() {
        assert_proc_non_disclosure(token_value());
    }

    fn assert_proc_non_disclosure(secret: &[u8]) {
        let cmdline = fs::read("/proc/self/cmdline").unwrap();
        let environ = fs::read("/proc/self/environ").unwrap();
        assert!(!cmdline.windows(secret.len()).any(|part| part == secret));
        assert!(!environ.windows(secret.len()).any(|part| part == secret));
        for entry in fs::read_dir("/proc/self/fd").unwrap() {
            let Ok(entry) = entry else { continue };
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            assert!(!target
                .as_os_str()
                .as_bytes()
                .windows(secret.len())
                .any(|part| part == secret));
        }
    }
}
