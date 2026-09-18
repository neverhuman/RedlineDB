//! Persist identity before execution so failed runs retain their denominator.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{RunConfig, engine::BinaryIdentity};

pub(super) struct Snapshot {
    path: PathBuf,
    value: Value,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn git(args: &[&str]) -> Value {
    match Command::new("git").args(args).output() {
        Ok(output) if output.status.success() => {
            json!({"value": String::from_utf8_lossy(&output.stdout).trim_end(), "error": null})
        }
        Ok(output) => json!({"value": null, "error": String::from_utf8_lossy(&output.stderr)}),
        Err(error) => json!({"value": null, "error": error.to_string()}),
    }
}

impl Snapshot {
    pub fn begin(config: &RunConfig) -> Result<Self> {
        let executable = std::env::current_exe().context("locate runner executable")?;
        let corpus = serde_json::to_vec(&super::catalog::all_cases()?)?;
        let selected = serde_json::to_vec(&super::catalog::selected_official_cases()?)?;
        let mut snapshot = Self {
            path: config.output.with_extension("provenance.json"),
            value: json!({
                "schema_version": 1, "stage": "pending", "result": null,
                "started_unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),
                "profile_id": "sqlite-3.53.1-app-v1", "comparison_policy_version": "2",
                "profile_manifest_sha256": hash(include_bytes!("../../profiles/sqlite-3.53.1-app-v1.json")),
                "corpus_serialization": "serde_json Case array, catalog order, including default fields",
                "corpus_sha256": hash(&corpus), "selected_cases_sha256": hash(&selected),
                "git_commit": git(&["rev-parse", "HEAD"]),
                "git_dirty": git(&["status", "--porcelain=v1", "--untracked-files=all"]),
                "platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                    "family": std::env::consts::FAMILY},
                "runner": {"path": executable, "sha256": hash(&fs::read(&executable)?)},
                "reference": null, "target": null, "oracle_receipt_sha256": null,
                "configuration": {"reference_bin": config.reference_bin, "target_bin": config.target_bin,
                    "output": config.output, "tmp_root": config.tmp_root, "workers": config.workers,
                    "repetitions": config.repetitions, "warmup": config.warmup,
                    "memory_samples": config.memory_samples,
                    "qualify_profile": std::env::var_os("REDLINE_TESTING_QUALIFY_PROFILE").is_some()},
                "limitation": "Dirty source snapshot is not proof of binary build inputs; identities describe executed artifacts."
            }),
        };
        snapshot.persist()?;
        Ok(snapshot)
    }

    pub fn identity(&mut self, role: &str, identity: &BinaryIdentity) -> Result<()> {
        self.value[role] = json!({"path": identity.executable_path,
            "sha256": identity.executable_sha256, "version": identity.version});
        if role == "reference" {
            let prefix = Path::new(&identity.executable_path)
                .parent()
                .and_then(Path::parent);
            if let Some(prefix) = prefix {
                let path = prefix.join("oracle-identity.json");
                self.value["oracle_receipt_path"] = json!(path);
                match fs::read(path) {
                    Ok(bytes) => self.value["oracle_receipt_sha256"] = json!(hash(&bytes)),
                    Err(error) => self.value["oracle_receipt_error"] = json!(error.to_string()),
                }
            }
        }
        self.persist()
    }

    pub fn start_output(&mut self, output: &Path) -> Result<()> {
        // Match CLI replacement semantics even for direct run() callers. Never
        // attach an older run's raw output to a preflight failure.
        fs::write(output, []).context("initialize this run's raw output")?;
        self.value["raw_output_initialized"] = json!(true);
        self.persist()
    }

    pub fn finish(&mut self, output: &Path, error: Option<String>) -> Result<()> {
        self.value["stage"] = json!(if error.is_some() {
            "failed"
        } else {
            "completed"
        });
        self.value["result"] = json!({"error": error});
        self.value["finished_unix_seconds"] = json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        );
        if self.value["raw_output_initialized"] != true {
            self.value["raw_output_error"] = json!("this run did not initialize raw output");
            return self.persist();
        }
        match fs::read(output) {
            Ok(bytes) => self.value["raw_output_sha256"] = json!(hash(&bytes)),
            Err(error) => self.value["raw_output_error"] = json!(error.to_string()),
        }
        self.persist()
    }

    fn persist(&mut self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let pending = self.path.with_extension("json.pending");
        fs::write(&pending, serde_json::to_vec_pretty(&self.value)?)?;
        fs::rename(&pending, &self.path).context("publish SQLite run provenance")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_bind_content_and_failed_runs_keep_initial_identity() {
        assert_eq!(
            hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(hash(b"abc"), hash(b"abcd"));
        let dir = std::env::temp_dir().join(format!("redline-provenance-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut snapshot = Snapshot {
            path: dir.join("run.provenance.json"),
            value: json!({"stage":"pending", "corpus_sha256":hash(b"fixture")}),
        };
        snapshot.persist().unwrap();
        let pending: Value = serde_json::from_slice(&fs::read(&snapshot.path).unwrap()).unwrap();
        assert_eq!(pending["stage"], "pending");
        let raw = dir.join("run.jsonl");
        fs::write(&raw, b"older run").unwrap();
        snapshot.start_output(&raw).unwrap();
        assert!(fs::read(&raw).unwrap().is_empty());
        fs::write(&raw, b"partial diagnostics").unwrap();
        snapshot
            .finish(&raw, Some("oracle mismatch".into()))
            .unwrap();
        let failed: Value = serde_json::from_slice(&fs::read(&snapshot.path).unwrap()).unwrap();
        assert_eq!(failed["stage"], "failed");
        assert_eq!(failed["corpus_sha256"], pending["corpus_sha256"]);
        assert_eq!(failed["raw_output_sha256"], hash(b"partial diagnostics"));
        assert_eq!(failed["result"]["error"], "oracle mismatch");
        fs::remove_dir_all(dir).unwrap();
    }
}
