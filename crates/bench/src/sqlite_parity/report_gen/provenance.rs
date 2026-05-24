use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::io::{display_path, sha256_file};

#[derive(Debug, Clone)]
pub(super) struct ValidatedProvenance {
    pub(super) path: String,
    pub(super) provenance_sha256: String,
    pub(super) raw_jsonl_sha256: String,
    pub(super) redline_testing_binary_sha256: String,
    pub(super) release_binary_sha256: Option<String>,
}

pub(super) fn validate_provenance(
    provenance_path: &Path,
    raw_path: &Path,
    raw_sha256: &str,
) -> Result<ValidatedProvenance> {
    let provenance_text = fs::read_to_string(provenance_path).with_context(|| {
        format!(
            "read sqlite parity provenance {}",
            provenance_path.display()
        )
    })?;
    let provenance: Value = serde_json::from_str(&provenance_text).with_context(|| {
        format!(
            "parse sqlite parity provenance {}",
            provenance_path.display()
        )
    })?;

    let output_hashes = collect_hash_maps(&provenance, &["output_file_hashes", "output_hashes"]);
    let Some(provenance_raw_sha256) = find_raw_jsonl_hash(&output_hashes, raw_path) else {
        bail!(
            "sqlite parity provenance {} lacks raw.jsonl sha256 in output_file_hashes/output_hashes",
            provenance_path.display()
        );
    };
    if provenance_raw_sha256 != raw_sha256 {
        bail!(
            "sqlite parity provenance raw.jsonl sha256 mismatch: provenance {} != actual {}",
            provenance_raw_sha256,
            raw_sha256
        );
    }

    let Some(redline_testing_binary_sha256) =
        field_sha256(&provenance, &REDLINE_TESTING_BINARY_SHA_FIELDS)
    else {
        bail!(
            "sqlite parity provenance {} lacks redline_testing_binary_sha256",
            provenance_path.display()
        );
    };

    let release_binary_sha256 = field_sha256(&provenance, &RELEASE_BINARY_SHA_FIELDS)
        .or_else(|| release_binary_hash_from_maps(&provenance));
    if let Some(release_binary_sha256) = &release_binary_sha256
        && *release_binary_sha256 != redline_testing_binary_sha256
    {
        bail!(
            "sqlite parity provenance redline-testing binary sha mismatch: redline_testing_binary_sha256 {} != release artifact/bin sha {}",
            redline_testing_binary_sha256,
            release_binary_sha256
        );
    }

    Ok(ValidatedProvenance {
        path: display_path(provenance_path).to_string(),
        provenance_sha256: sha256_file(provenance_path)?,
        raw_jsonl_sha256: raw_sha256.to_owned(),
        redline_testing_binary_sha256,
        release_binary_sha256,
    })
}

const REDLINE_TESTING_BINARY_SHA_FIELDS: &[&[&str]] = &[
    &["redline_testing_binary_sha256"],
    &["redline_testing", "binary_sha256"],
    &["redline_testing", "sha256"],
    &["tool", "redline_testing_binary_sha256"],
    &["tool", "binary_sha256"],
];

const RELEASE_BINARY_SHA_FIELDS: &[&[&str]] = &[
    &["redline_testing_release_binary_sha256"],
    &["redline_testing_release_artifact_binary_sha256"],
    &["release_artifact_binary_sha256"],
    &["release_artifact_bin_sha256"],
    &["installed_release_binary_sha256"],
    &["installed_release_artifact_binary_sha256"],
    &["redline_testing", "release_binary_sha256"],
    &["redline_testing", "release_artifact_binary_sha256"],
    &["redline_testing", "release_artifact", "binary_sha256"],
    &["redline_testing", "release_artifact", "bin_sha256"],
    &["release_artifact", "binary_sha256"],
    &["release_artifact", "bin_sha256"],
    &["installed_release_artifact", "binary_sha256"],
    &["installed_release_artifact", "bin_sha256"],
    &["release", "binary_sha256"],
    &["release", "bin_sha256"],
];

fn collect_hash_maps(value: &Value, field_names: &[&str]) -> BTreeMap<String, String> {
    let mut hashes = BTreeMap::new();
    for field_name in field_names {
        let Some(field) = value.get(*field_name) else {
            continue;
        };
        collect_hash_entries(field, &mut hashes);
    }
    hashes
}

fn collect_hash_entries(value: &Value, hashes: &mut BTreeMap<String, String>) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if let (Some(path), Some(hash)) = (entry_path(value), hash_value(value)) {
                hashes.insert(normalize_path_key(&path), hash);
            } else if let Some(hash) = hash_value(value) {
                hashes.insert(normalize_path_key(key), hash);
            }
        }
    } else if let Some(entries) = value.as_array() {
        for entry in entries {
            if let (Some(path), Some(hash)) = (entry_path(entry), hash_value(entry)) {
                hashes.insert(normalize_path_key(&path), hash);
            }
        }
    }
}

fn find_raw_jsonl_hash(hashes: &BTreeMap<String, String>, raw_path: &Path) -> Option<String> {
    let raw_path = normalize_path_key(&display_path(raw_path));
    for candidate in [raw_path.as_str(), "raw.jsonl"] {
        if let Some(hash) = hashes.get(candidate) {
            return Some(hash.clone());
        }
    }
    hashes
        .iter()
        .find(|(path, _)| path.ends_with("/raw.jsonl"))
        .map(|(_, hash)| hash.clone())
}

fn field_sha256(value: &Value, fields: &[&[&str]]) -> Option<String> {
    fields
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(hash_string))
}

fn release_binary_hash_from_maps(value: &Value) -> Option<String> {
    let hashes = collect_hash_maps(
        value,
        &[
            "release_file_hashes",
            "release_artifact_hashes",
            "artifact_file_hashes",
            "input_file_hashes",
        ],
    );
    hashes
        .iter()
        .find(|(path, _)| path.ends_with("bin/redline-testing"))
        .map(|(_, hash)| hash.clone())
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn entry_path(value: &Value) -> Option<String> {
    ["path", "file", "name"]
        .iter()
        .find_map(|field| value.get(*field).and_then(Value::as_str))
        .map(str::to_owned)
}

fn hash_value(value: &Value) -> Option<String> {
    hash_string(value).or_else(|| {
        ["sha256", "hash", "digest"]
            .iter()
            .find_map(|field| value.get(*field).and_then(hash_string))
    })
}

fn hash_string(value: &Value) -> Option<String> {
    value.as_str().and_then(normalize_sha256)
}

fn normalize_sha256(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    let value = value.strip_prefix("sha256:").unwrap_or(&value).to_owned();
    (value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(value)
}

fn normalize_path_key(path: &str) -> String {
    path.trim().replace('\\', "/")
}
