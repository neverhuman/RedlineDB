use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

#[derive(Debug)]
struct Config {
    out_dir: PathBuf,
    entrypoint: String,
    workers: u64,
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let config = parse_args(args)?;
    let repo_root =
        std::env::current_dir().context("artifact-metadata: resolve repository root")?;
    let sha = git_output(&repo_root, &["rev-parse", "HEAD"])?;
    let tree = git_output(&repo_root, &["rev-parse", "HEAD^{tree}"])?;
    let tracked = git_output(&repo_root, &["ls-files"])?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let repo = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("artifact-metadata: repository name is not valid UTF-8"))?;
    let generated_at = git_output(&repo_root, &["show", "-s", "--format=%cI", "HEAD"])?;
    let documents = documents(
        repo,
        &sha,
        &tree,
        &generated_at,
        config.workers,
        &config.entrypoint,
        tracked,
    );

    fs::create_dir_all(config.out_dir.join("receipts")).with_context(|| {
        format!(
            "artifact-metadata: create {}",
            config.out_dir.join("receipts").display()
        )
    })?;
    write_json(&config.out_dir.join("context.json"), &documents.context)?;
    write_json(&config.out_dir.join("manifest.json"), &documents.manifest)?;
    write_json(
        &config.out_dir.join("receipts/local-ci.json"),
        &documents.receipt,
    )?;
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Config> {
    let mut out_dir = None;
    let mut entrypoint = None;
    let mut workers = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| anyhow!("artifact-metadata: {} requires a value", args[index]))?;
        match args[index].as_str() {
            "--out-dir" => out_dir = Some(PathBuf::from(value)),
            "--entrypoint" => entrypoint = Some(value.clone()),
            "--workers" => {
                workers = Some(
                    value
                        .parse::<u64>()
                        .with_context(|| format!("artifact-metadata: invalid workers {value:?}"))?,
                )
            }
            option => bail!("artifact-metadata: unknown option {option:?}"),
        }
        index += 2;
    }
    Ok(Config {
        out_dir: out_dir.ok_or_else(|| anyhow!("artifact-metadata: --out-dir is required"))?,
        entrypoint: entrypoint
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("artifact-metadata: --entrypoint is required"))?,
        workers: workers.ok_or_else(|| anyhow!("artifact-metadata: --workers is required"))?,
    })
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .with_context(|| format!("artifact-metadata: execute git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "artifact-metadata: git {} failed with {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout)
        .context("artifact-metadata: git output is not valid UTF-8")
        .map(|value| value.trim_end_matches('\n').to_owned())
}

struct Documents {
    context: Value,
    manifest: Value,
    receipt: Value,
}

#[allow(clippy::too_many_arguments)]
fn documents(
    repo: &str,
    sha: &str,
    tree: &str,
    generated_at: &str,
    workers: u64,
    entrypoint: &str,
    tracked_files: Vec<String>,
) -> Documents {
    Documents {
        context: json!({
            "schema_version": 1,
            "generated_by": "ops/ci/artifact_support.sh",
            "repo": repo,
            "sha": sha,
            "tree": tree,
            "generated_at": generated_at,
            "workers": workers,
            "ci_entrypoint": entrypoint,
        }),
        manifest: json!({
            "schema_version": 1,
            "sha": sha,
            "tracked_file_count": tracked_files.len(),
            "tracked_files": tracked_files,
        }),
        receipt: json!({
            "schema_version": 1,
            "sha": sha,
            "entrypoint": entrypoint,
            "status": "success",
        }),
    }
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, json_bytes(value)?)
        .with_context(|| format!("artifact-metadata: write {}", path.display()))
}

fn json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec_pretty(value).context("artifact-metadata: serialize JSON")?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_all_artifact_documents() {
        let documents = documents(
            "redline",
            &"a".repeat(40),
            &"b".repeat(40),
            "2026-07-12T00:00:00Z",
            40,
            "just fast",
            vec!["Cargo.lock".to_owned(), "Cargo.toml".to_owned()],
        );
        assert_eq!(documents.context["workers"], 40);
        assert_eq!(
            documents.context["generated_by"],
            "ops/ci/artifact_support.sh"
        );
        assert_eq!(documents.manifest["tracked_file_count"], 2);
        assert_eq!(documents.manifest["tracked_files"][1], "Cargo.toml");
        assert_eq!(documents.receipt["status"], "success");
        assert_eq!(
            String::from_utf8(json_bytes(&documents.receipt).unwrap()).unwrap(),
            concat!(
                "{\n",
                "  \"entrypoint\": \"just fast\",\n",
                "  \"schema_version\": 1,\n",
                "  \"sha\": \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\n",
                "  \"status\": \"success\"\n",
                "}\n"
            )
        );
    }

    #[test]
    fn rejects_missing_and_invalid_arguments() {
        assert!(
            parse_args(&[])
                .unwrap_err()
                .to_string()
                .contains("--out-dir")
        );
        let error = parse_args(&[
            "--out-dir".to_owned(),
            "out".to_owned(),
            "--entrypoint".to_owned(),
            "just fast".to_owned(),
            "--workers".to_owned(),
            "many".to_owned(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("invalid workers"));
    }
}
