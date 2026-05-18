//! JSON serialization helpers and version-index refresh logic.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::read::read_json_file;

pub(crate) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir -p {}: {}", parent.display(), e))?;
    }
    // Python uses `json.dump(value, ..., indent=2, sort_keys=True)`
    // followed by a trailing newline. `serde_json::to_string_pretty`
    // uses 2-space indent; serde_json::Map preserves insertion order so
    // we sort by re-serializing through a BTreeMap.
    let sorted = sort_value_keys(value);
    let text = serde_json::to_string_pretty(&sorted)
        .map_err(|e| format!("serialize {}: {}", path.display(), e))?;
    let mut file =
        fs::File::create(path).map_err(|e| format!("create {}: {}", path.display(), e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    file.write_all(b"\n")
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(())
}

/// Recursively walk a JSON value and rebuild every object as a
/// BTreeMap-backed `serde_json::Map`, so the final serialization is
/// sorted by key like Python's `sort_keys=True`.
fn sort_value_keys(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), sort_value_keys(v));
            }
            // Build a serde_json::Map preserving the BTreeMap order.
            let mut out = Map::with_capacity(sorted.len());
            for (k, v) in sorted {
                out.insert(k, v);
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_value_keys).collect()),
        _ => value.clone(),
    }
}

pub(crate) fn refresh_index(version_dir: &Path, git_sha: &str) -> Result<PathBuf, String> {
    let mut reports: Vec<Value> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    if version_dir.exists() {
        for entry in fs::read_dir(version_dir)
            .map_err(|e| format!("read_dir {}: {}", version_dir.display(), e))?
        {
            let entry = entry.map_err(|e| format!("dir entry error: {e}"))?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if p.file_name().and_then(|s| s.to_str()) == Some("index.json") {
                continue;
            }
            paths.push(p);
        }
    }
    paths.sort();
    for p in paths {
        let data = read_json_file(&p)?;
        let mut entry = Map::new();
        entry.insert(
            "file".to_string(),
            Value::String(
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
        );
        entry.insert(
            "stamp".to_string(),
            data.get("stamp").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "suite".to_string(),
            data.get("suite").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "profile".to_string(),
            data.get("profile").cloned().unwrap_or(Value::Null),
        );
        entry.insert(
            "workloads".to_string(),
            data.get("workloads")
                .cloned()
                .unwrap_or(Value::Array(Vec::new())),
        );
        entry.insert(
            "config_path".to_string(),
            data.get("config_path").cloned().unwrap_or(Value::Null),
        );
        reports.push(Value::Object(entry));
    }
    let index = json!({
        "git_sha": git_sha,
        "generated_at_utc": iso_utc_now(),
        "reports": reports,
    });
    let index_path = version_dir.join("index.json");
    write_json(&index_path, &index)?;
    Ok(index_path)
}

/// Emit the current UTC time as an ISO-8601 string with microsecond
/// precision and a `+00:00` suffix, matching Python's
/// `datetime.now(timezone.utc).isoformat()`.
pub(crate) fn iso_utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs() as i64;
    let micros = (now.subsec_micros()) as u32;
    iso_utc_format(total_secs, micros)
}

/// Format a Unix epoch (seconds + microseconds) into the same string
/// shape Python emits: `YYYY-MM-DDTHH:MM:SS.uuuuuu+00:00` (or no
/// `.uuuuuu` if microseconds are zero, also matching Python).
pub(crate) fn iso_utc_format(epoch_secs: i64, micros: u32) -> String {
    let (year, month, day, hour, minute, second) = civil_from_epoch(epoch_secs);
    if micros == 0 {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
            year, month, day, hour, minute, second
        )
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            year, month, day, hour, minute, second, micros
        )
    }
}

/// Convert a Unix epoch in UTC seconds to a (Y, M, D, h, m, s) tuple.
/// Algorithm from Howard Hinnant's date library, ported to integer math.
fn civil_from_epoch(epoch_secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let mut secs = epoch_secs % 86_400;
    let mut days = epoch_secs.div_euclid(86_400);
    if secs < 0 {
        secs += 86_400;
        days -= 1;
    }
    let hour = (secs / 3600) as u32;
    let minute = ((secs % 3600) / 60) as u32;
    let second = (secs % 60) as u32;

    // Shift epoch so that day 0 is March 1, year 0 (Hinnant's "days from civil").
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_utc_format_matches_python_isoformat() {
        // 2024-01-02T03:04:05+00:00, no fractional part.
        let secs: i64 = 1704164645;
        assert_eq!(iso_utc_format(secs, 0), "2024-01-02T03:04:05+00:00");
        // Same instant with microseconds.
        assert_eq!(
            iso_utc_format(secs, 123456),
            "2024-01-02T03:04:05.123456+00:00"
        );
        // Leap year boundary.
        assert_eq!(iso_utc_format(1709251200, 0), "2024-03-01T00:00:00+00:00");
    }

    #[test]
    fn sort_value_keys_sorts_nested_objects() {
        let v = json!({
            "z": 1,
            "a": { "y": 2, "b": 3 }
        });
        let sorted = sort_value_keys(&v);
        let s = serde_json::to_string(&sorted).unwrap();
        // Top-level keys come out sorted; nested too.
        assert!(s.find("\"a\"").unwrap() < s.find("\"z\"").unwrap());
        assert!(s.find("\"b\"").unwrap() < s.find("\"y\"").unwrap());
    }
}
