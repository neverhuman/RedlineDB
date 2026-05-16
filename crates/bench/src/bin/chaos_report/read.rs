//! Loading raw records and manifest JSON from a stamp directory.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

pub(crate) fn repo_git_sha(repo: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("git rev-parse failed to spawn: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git rev-parse exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let s = String::from_utf8(out.stdout).map_err(|e| format!("git output not utf-8: {e}"))?;
    Ok(s.trim_end_matches('\n').to_string())
}

pub(crate) fn read_json_file(path: &Path) -> Result<Value, String> {
    let mut buf = String::new();
    fs::File::open(path)
        .map_err(|e| format!("open {}: {}", path.display(), e))?
        .read_to_string(&mut buf)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;
    serde_json::from_str(&buf).map_err(|e| format!("parse {}: {}", path.display(), e))
}

pub(crate) fn load_records(stamp_dir: &Path) -> Result<Vec<Value>, String> {
    let raw_dir = stamp_dir.join("raw");
    if !raw_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in
        fs::read_dir(&raw_dir).map_err(|e| format!("read_dir {}: {}", raw_dir.display(), e))?
    {
        let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let record_path = p.join("record.json");
        if record_path.exists() {
            paths.push(record_path);
        }
    }
    // Match Python: `sorted(glob.glob(...))` sorts lexicographically.
    paths.sort();
    let mut records = Vec::with_capacity(paths.len());
    for p in paths {
        let mut record = read_json_file(&p)?;
        if let Value::Object(ref mut map) = record {
            let run_name = match p.parent().and_then(Path::file_name) {
                Some(name) => name.to_string_lossy().into_owned(),
                None => String::new(),
            };
            map.insert(
                "_path".to_string(),
                Value::String(p.to_string_lossy().into_owned()),
            );
            map.insert("_run".to_string(), Value::String(run_name));
            records.push(record);
        } else {
            return Err(format!("record at {} is not a JSON object", p.display()));
        }
    }
    Ok(records)
}

pub(crate) fn load_manifest(stamp_dir: &Path) -> Result<Option<Value>, String> {
    let path = stamp_dir.join("manifest.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_json_file(&path)?))
}
