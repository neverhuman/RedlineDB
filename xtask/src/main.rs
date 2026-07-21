//! xtask — internal dev tools for redline-testing.
//!
//! Subcommands include:
//!
//!   * `generate` writes the matrix-product shards under
//!     `corpus/sqlite_parity/cases/gen_*.json`. Each shard is produced by a
//!     Rust generator: it enumerates a cartesian product of axes (functions,
//!     inputs, modifiers, etc.), runs the resulting SQL through `sqlite3
//!     -batch -bail :memory:`, captures the actual stdout + exit code, and
//!     emits a JSON shard with that captured output as expected_stdout. By
//!     construction every generated case passes the reference self-compare.
//!
//!   * `generate --check` re-generates each shard in memory and asserts byte
//!     equality with the on-disk shard. Used in CI to keep rules and shards
//!     in sync.
//!
//!   * `ship-gate` walks `corpus/sqlite_parity/cases/*.json`, runs each case
//!     through `sqlite3`, and verifies its declared expected_stdout, exit
//!     code, and substring contains-checks match. Failing case IDs are
//!     printed so they can be removed from the shard. This is the
//!     authoritative ship contract for the SQLite-parity corpus.
//!
//!   * Repository CI helpers update the score badge and emit the cost,
//!     readiness, artifact-support, and canary-telemetry JSON contracts.

mod badge;
mod custody;
mod docker_stage;
mod generators;
mod repo_ops;
mod sqlite_runner;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "redline-testing dev tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Update the marked Jankurai badge block in README.md from the tracked
    /// agent/jankurai-badge.json source of truth.
    UpdateBadge,
    /// Validate the zero-spend policy and emit its machine-readable receipt.
    CostBudget,
    /// Validate launch-gate documentation and emit its readiness receipt.
    ReleaseReadiness,
    /// Validate that a Jain corrective tag matches Cargo.toml's product version.
    ValidateReleaseTag {
        #[arg(long)]
        tag: String,
    },
    /// Write the artifact-support context, manifest, and local-CI receipt.
    ArtifactSupportJson {
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long)]
        entrypoint: String,
        #[arg(long)]
        sha: String,
        #[arg(long)]
        tree: String,
        #[arg(long)]
        generated_at: String,
        #[arg(long)]
        workers: usize,
    },
    /// Probe SignRail receipts and emit Jeryu canary telemetry as JSON.
    Telemetry {
        #[arg(long)]
        repo: String,
        #[arg(long)]
        sha: String,
        #[arg(long)]
        ring_percent: u8,
        #[arg(long)]
        slug: String,
        #[arg(long)]
        store_root: PathBuf,
    },
    /// Regenerate the matrix-product SQLite-parity shards into
    /// corpus/sqlite_parity/cases/gen_*.json. With --check, refuses to
    /// write and instead fails if the on-disk shard differs.
    Generate {
        #[arg(long)]
        check: bool,
        /// Only regenerate the named rule (e.g. "math", "cast"). Default: all.
        #[arg(long)]
        only: Option<String>,
        /// Path to the SQLite binary used to capture expected outputs.
        #[arg(long, default_value = "sqlite3")]
        sqlite_bin: String,
    },
    /// Validate every case in every shard by running it through sqlite3 and
    /// asserting the declared expected behavior. Exits non-zero with the
    /// list of failing case IDs if anything diverges.
    ShipGate {
        /// Optional path to a single shard JSON; default: every shard under
        /// corpus/sqlite_parity/cases/.
        path: Option<PathBuf>,
        #[arg(long, default_value = "sqlite3")]
        sqlite_bin: String,
    },
    /// Stage frozen oracle artifacts, validate the locked Cargo closure, build
    /// offline, and emit a content-addressed in-tree custody receipt.
    CustodyStage {
        /// Existing local cache used only to bootstrap exact custody bytes.
        #[arg(long)]
        source_cargo_home: PathBuf,
        #[arg(long)]
        cargo_home: PathBuf,
        #[arg(long)]
        sqlite_bin: PathBuf,
        #[arg(long)]
        postgres_client_bin: PathBuf,
        #[arg(long)]
        postgres_server_bin: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Build a symlink-free, content-addressed Docker runtime context from
    /// exact local custody. This command never pulls or builds an image.
    DockerStage {
        #[arg(long)]
        source_cargo_home: PathBuf,
        #[arg(long)]
        source_custody_sha256: String,
        #[arg(long)]
        cargo_home: PathBuf,
        #[arg(long)]
        sqlite_bin: PathBuf,
        #[arg(long)]
        psql_bin: PathBuf,
        #[arg(long)]
        postgres_bin: PathBuf,
        #[arg(long)]
        target_bin: PathBuf,
        #[arg(long)]
        core_commit: String,
        #[arg(long)]
        core_tree: String,
        #[arg(long, default_value = "docker/images.lock.toml")]
        image_manifest: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo_root = find_repo_root()?;
    match cli.command {
        Command::UpdateBadge => badge::run(&repo_root),
        Command::CostBudget => repo_ops::cost_budget(&repo_root),
        Command::ReleaseReadiness => repo_ops::release_readiness(&repo_root),
        Command::ValidateReleaseTag { tag } => repo_ops::validate_release_tag(&repo_root, &tag),
        Command::ArtifactSupportJson {
            out_dir,
            entrypoint,
            sha,
            tree,
            generated_at,
            workers,
        } => repo_ops::artifact_support_json(
            &repo_root,
            &out_dir,
            &entrypoint,
            &sha,
            &tree,
            &generated_at,
            workers,
        ),
        Command::Telemetry {
            repo,
            sha,
            ring_percent,
            slug,
            store_root,
        } => repo_ops::telemetry(&repo, &sha, ring_percent, &slug, &store_root),
        Command::Generate {
            check,
            only,
            sqlite_bin,
        } => generators::run(&repo_root, &sqlite_bin, only.as_deref(), check),
        Command::ShipGate { path, sqlite_bin } => {
            let target = path
                .unwrap_or_else(|| repo_root.join("corpus").join("sqlite_parity").join("cases"));
            ship_gate(&target, &sqlite_bin)
        }
        Command::CustodyStage {
            source_cargo_home,
            cargo_home,
            sqlite_bin,
            postgres_client_bin,
            postgres_server_bin,
            out_dir,
        } => custody::stage(
            &repo_root,
            &source_cargo_home,
            &cargo_home,
            &sqlite_bin,
            &postgres_client_bin,
            &postgres_server_bin,
            &out_dir,
        ),
        Command::DockerStage {
            source_cargo_home,
            source_custody_sha256,
            cargo_home,
            sqlite_bin,
            psql_bin,
            postgres_bin,
            target_bin,
            core_commit,
            core_tree,
            image_manifest,
            out_dir,
        } => docker_stage::stage(
            &repo_root,
            &source_cargo_home,
            &source_custody_sha256,
            &cargo_home,
            &sqlite_bin,
            &psql_bin,
            &postgres_bin,
            &target_bin,
            &core_commit,
            &core_tree,
            &image_manifest,
            &out_dir,
        ),
    }
}

fn ship_gate(target: &Path, sqlite_bin: &str) -> Result<()> {
    let paths = if target.is_dir() {
        let mut paths = Vec::new();
        for entry in fs::read_dir(target).with_context(|| format!("read {}", target.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_'))
            {
                paths.push(path);
            }
        }
        paths.sort();
        paths
    } else if target.is_file() {
        vec![target.to_path_buf()]
    } else {
        bail!("not a file or directory: {}", target.display());
    };

    let mut total = 0usize;
    let mut failures: Vec<(String, u64, String, String)> = Vec::new();
    for shard in &paths {
        let body =
            fs::read_to_string(shard).with_context(|| format!("read shard {}", shard.display()))?;
        let cases: Vec<sqlite_runner::Case> = serde_json::from_str(&body)
            .with_context(|| format!("parse shard {}", shard.display()))?;
        for case in &cases {
            total += 1;
            if let Err(reason) = sqlite_runner::validate(case, sqlite_bin) {
                failures.push((
                    shard
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    case.id,
                    case.name.clone(),
                    reason.to_string(),
                ));
            }
        }
    }
    println!("ship-gate: {total} cases, {} failures", failures.len());
    for (shard, id, name, reason) in &failures {
        println!("  FAIL {shard} #{id} {name}: {reason}");
    }
    if failures.is_empty() {
        Ok(())
    } else {
        bail!("{} cases failed ship-gate", failures.len())
    }
}

fn find_repo_root() -> Result<PathBuf> {
    // xtask is invoked from anywhere under the workspace; walk up looking
    // for the Cargo.toml that declares the `redline-testing` package or
    // workspace.
    let mut cwd = std::env::current_dir().context("cwd")?;
    loop {
        if cwd.join("corpus").join("sqlite_parity").is_dir() && cwd.join("xtask").is_dir() {
            return Ok(cwd);
        }
        if !cwd.pop() {
            bail!("could not locate redline-testing repo root from cwd");
        }
    }
}
