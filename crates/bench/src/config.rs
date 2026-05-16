use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "redlinedb-bench")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Compare(CompareArgs),
    Certify(CertifyArgs),
    // Cross-engine replay suite: walks a `.sqlt` corpus against every engine
    // and asserts byte-identical outputs.
    CrossEngine(CrossEngineArgs),
    Recover(RecoverArgs),
    RecoverMatrix(RecoverMatrixArgs),
    FailpointMatrix(FailpointMatrixArgs),
    Gates(GatesArgs),
    #[command(hide = true)]
    RecoverChild(RecoverChildArgs),
    #[command(hide = true)]
    FailpointChild(FailpointChildArgs),
}

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, value_enum)]
    pub engine: EngineKind,
    #[arg(long, value_enum)]
    pub workload: WorkloadKind,
    #[arg(long, value_enum, default_value = "strict")]
    pub durability: DurabilityKind,
    #[arg(long, default_value_t = 1)]
    pub threads: usize,
    #[arg(long, default_value_t = 512)]
    pub rows: usize,
    #[arg(long, default_value_t = 2)]
    pub seconds: u64,
    #[arg(long, default_value_t = 16)]
    pub cache_mib: usize,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    #[arg(long)]
    pub out: Option<PathBuf>,
}

impl RunArgs {
    pub fn into_run_spec(&self) -> Result<RunSpec> {
        let duration = Duration::from_secs(self.seconds.max(1));
        Ok(RunSpec {
            engine: self.engine,
            workload: self.workload,
            durability: self.durability,
            threads: self.threads.max(1),
            rows: self.rows.max(1),
            duration,
            cache_bytes: self.cache_mib.max(1) * 1024 * 1024,
            seed: self.seed,
            base_dir: std::env::temp_dir().join("redlinedb-bench"),
        })
    }
}

#[derive(Debug, Clone, Args)]
pub struct CompareArgs {
    #[arg(long)]
    pub config: PathBuf,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub report: Option<PathBuf>,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
}

#[derive(Debug, Clone, Args)]
pub struct CertifyArgs {
    #[arg(long)]
    pub config: PathBuf,
    #[arg(long)]
    pub out_dir: PathBuf,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    #[arg(long, default_value_t = 1)]
    pub repetitions: usize,
    #[arg(long, default_value_t = 0)]
    pub warmup: usize,
    /// Wrap each per-engine bench child with `strace -c` (Linux only) to
    /// collect a syscall summary alongside the run. Setting the
    /// `REDLINEDB_BENCH_WITH_STRACE` environment variable to a non-empty
    /// value has the same effect; the flag and the env var OR together,
    /// so either trigger enables capture.
    #[arg(long, default_value_t = false)]
    pub with_strace: bool,
}

impl CertifyArgs {
    /// True when strace capture should run for this invocation. Either the
    /// `--with-strace` flag or a non-empty `REDLINEDB_BENCH_WITH_STRACE`
    /// env var is sufficient — never both required.
    pub fn strace_enabled(&self) -> bool {
        self.with_strace
            || std::env::var_os("REDLINEDB_BENCH_WITH_STRACE")
                .filter(|value| !value.is_empty())
                .is_some()
    }
}

#[derive(Debug, Clone, Args)]
pub struct CrossEngineArgs {
    #[arg(long, value_enum, default_value = "both")]
    pub engine: EngineSet,
    #[arg(long)]
    pub test_dir: PathBuf,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct RecoverArgs {
    #[arg(long, value_enum, default_value = "both")]
    pub engine: EngineSet,
    #[arg(long, value_enum, default_value = "single-row-insert")]
    pub workload: WorkloadKind,
    #[arg(long, value_enum, default_value = "strict")]
    pub durability: DurabilityKind,
    #[arg(long, default_value_t = 2)]
    pub seconds: u64,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct RecoverMatrixArgs {
    #[arg(long, value_enum, default_value = "both")]
    pub engine: EngineSet,
    #[arg(long)]
    pub config: PathBuf,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct GatesArgs {
    #[arg(long)]
    pub input: PathBuf,
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct RecoverChildArgs {
    #[arg(long, value_enum)]
    pub engine: EngineKind,
    #[arg(long, value_enum)]
    pub durability: DurabilityKind,
    #[arg(long, value_enum, default_value = "wal")]
    pub scenario: RecoveryScenarioKind,
    #[arg(long)]
    pub db_dir: PathBuf,
    #[arg(long)]
    pub ack_log: PathBuf,
    #[arg(long, default_value_t = 1024)]
    pub rows: usize,
    #[arg(long, default_value_t = 32)]
    pub checkpoint_every_rows: usize,
}

#[derive(Debug, Clone, Args)]
pub struct FailpointMatrixArgs {
    #[arg(long)]
    pub config: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, default_value_t = 7)]
    pub seed: u64,
}

#[derive(Debug, Clone, Args)]
pub struct FailpointChildArgs {
    #[arg(long, value_enum)]
    pub engine: EngineKind,
    #[arg(long, value_enum)]
    pub durability: DurabilityKind,
    #[arg(long)]
    pub db_dir: PathBuf,
    #[arg(long)]
    pub ack_log: PathBuf,
    /// Name of the failpoint to arm (e.g. `engine::commit::before_publish`).
    #[arg(long)]
    pub failpoint: String,
    /// Action to inject. Currently `panic`, `return`, or `abort` are
    /// recognised; everything else is forwarded verbatim to `fail::cfg`.
    #[arg(long)]
    pub action: String,
    /// Number of rows to attempt. The child returns naturally if it
    /// finishes the workload before the failpoint fires.
    #[arg(long, default_value_t = 1024)]
    pub rows: usize,
    /// Stop arming the failpoint after this many hits, then exit cleanly.
    /// Used for `kill_after_n_hits` cases that survive the configured
    /// number of fires without dying.
    #[arg(long, default_value_t = 1)]
    pub kill_after_n_hits: u64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum EngineKind {
    Redline,
    Sqlite,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum EngineSet {
    Redline,
    Sqlite,
    Both,
}

impl EngineSet {
    pub fn expand(self) -> &'static [EngineKind] {
        match self {
            Self::Redline => &[EngineKind::Redline],
            Self::Sqlite => &[EngineKind::Sqlite],
            Self::Both => &[EngineKind::Redline, EngineKind::Sqlite],
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum WorkloadKind {
    SingleRowInsert,
    BatchedInsert100,
    PointReadPk,
    SecondaryIndexRead,
    SecondaryIndexRange,
    WritersDisjoint,
    HotRowUpdate,
    MixedOltp,
    Mixed95Read5Write,
    Mixed80Read20Write,
    Mixed50Read50Write,
    /// Lane BH P1 #7: binary-search the engine for its maximum
    /// stable concurrent-connection count. The workload manages
    /// its own connection pool — the harness's `--threads` arg is
    /// ignored. Output is a single record with
    /// `engine_stats.max_stable_connections` populated.
    ConnectionLimit,
    /// Lane VE: forces the spillable-sort path. Inserts `rows` (default
    /// 200_000 with a 64-byte payload) and repeatedly issues
    /// `SELECT * FROM t ORDER BY payload` with a tight `work_mem_bytes`,
    /// surfacing `engine_stats.spill_bytes_ratio`.
    LargeSortSpill,
    /// Phase 10 cert-v3: read-heavy SQLite JSON1 path extraction workload.
    JsonPathExtract,
    /// Phase 10 cert-v3: JSON path mutation workload using `json_set`.
    JsonPathUpdate,
    /// Phase 10 cert-v3: exact flat vector distance search.
    VectorFlatSearch,
    /// Phase 10 cert-v3: HNSW approximate vector search.
    VectorAnnSearch,
    /// Phase 10 cert-v3: DiskANN sector-backed approximate vector search.
    VectorAnnSearchDisk,
    /// Phase 10 cert-v3: many tiny commits to expose group-commit batching.
    CommitStormBatched,
    /// Phase 11 wave 1a: secondary index leaf-walk efficiency probe.
    /// `SELECT COUNT(*) FROM kv WHERE tenant BETWEEN ? AND ?` over the
    /// existing `kv_tenant_idx`. Fixture shape mirrors
    /// `SecondaryIndexRange` but the projection is purely the count
    /// aggregate so the engine never visits the heap, isolating the
    /// cost of walking the index leaves.
    ///
    /// Proof: crates/bench/tests/tenant_isolation.rs::dual_connection_cross_tenant_index_probe_yields_zero_rows
    /// (lines 243-302; assert cross-tenant index probe yields 0 rows;
    /// HLT-022-AUTHZ-ISOLATION-GAP). The test opens two distinct
    /// `Connection`s — one structurally scoped to tenant A, one to
    /// tenant B — and asserts `COUNT(*) FROM kv WHERE tenant = B == 0`
    /// after tenant A inserts 24 rows, including under a concurrent
    /// late write. Sibling deterministic scenarios in the same file:
    /// `owner_can_read` (lines 134-151, positive control),
    /// `non_owner_denied` (lines 153-194, denial via PK + index probe),
    /// `cross_tenant_index_probe_empty` (lines 196-241, equality + range
    /// + open-ended probes over the secondary index),
    /// `tombstone_owner_only` (lines 304-end, owner-scoped delete).
    /// Runnable via `rtk cargo test -p redlinedb-bench --test
    /// tenant_isolation --quiet --locked`. See also
    /// `agent/security-policy.toml` [[proofs]] entry for
    /// `HLT-022-AUTHZ-ISOLATION-GAP` for proof routing.
    SecondaryIndexCount,
    /// Phase 11 wave 1a: ordered range with `LIMIT` early-stop. The
    /// query shape is `SELECT * FROM kv WHERE tenant >= ? ORDER BY
    /// tenant LIMIT 100`, where the index leading column matches
    /// `ORDER BY` so the planner can stop after 100 rows.
    SecondaryIndexOrderedLimit,
    /// Phase 11 wave 1a: covering range scan with cold cache. A
    /// fresh database is opened per measurement so the query
    /// `SELECT k, v FROM covered_kv WHERE k BETWEEN ? AND ?` over the
    /// `(k, v)` covering index has to fault every leaf page in.
    CoveredRangeCold,
    /// Phase 11 wave 1a: same as `CoveredRangeCold` but with a
    /// warmup pass over the same range before the measurement window
    /// opens, so the leaf pages are already in cache.
    CoveredRangeWarm,
    /// Phase 11 wave 1a: hot-counter increment baseline. Single hot
    /// row updated as `UPDATE hot_counter SET counter = counter + 1
    /// WHERE pk = ?` where `counter` is non-indexed. Establishes
    /// the baseline for the future commutative-delta combiner path;
    /// distinct from `HotRowUpdate` which writes a general blob
    /// payload.
    HotCounterUpdate,
    /// Queue-like mixed read/write workload: concurrent producers
    /// append pending jobs, consumers claim the oldest pending job,
    /// and pollers count pending work.
    QueueMixed,
    /// Chaos suite: lock convoy under hot-write pressure.
    ChaosLockConvoy,
    /// Chaos suite: open a fresh connection per operation.
    ChaosConnectionChurn,
    /// Chaos suite: checkpoint storms while readers and writers run.
    ChaosCheckpointThrash,
    /// Chaos suite: mixed inserts, deletes, and indexed scans.
    ChaosIndexHammer,
    /// Chaos suite: sort/spill convoys under concurrent writes.
    ChaosSortSpillConvoy,
    /// Chaos suite: extreme-only DDL / schema churn.
    ChaosSchemaStorm,
}

impl WorkloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            // Strings here MUST round-trip through clap ValueEnum / serde
            // kebab-case (the CLI subcommand and the certify child both parse
            // them), and `clap::ValueEnum` auto-derives kebab-case directly
            // from the variant name (no inserted dashes around digits).
            Self::SingleRowInsert => "single-row-insert",
            Self::BatchedInsert100 => "batched-insert100",
            Self::PointReadPk => "point-read-pk",
            Self::SecondaryIndexRead => "secondary-index-read",
            Self::SecondaryIndexRange => "secondary-index-range",
            Self::WritersDisjoint => "writers-disjoint",
            Self::HotRowUpdate => "hot-row-update",
            Self::MixedOltp => "mixed-oltp",
            Self::Mixed95Read5Write => "mixed95-read5-write",
            Self::Mixed80Read20Write => "mixed80-read20-write",
            Self::Mixed50Read50Write => "mixed50-read50-write",
            Self::ConnectionLimit => "connection-limit",
            Self::LargeSortSpill => "large-sort-spill",
            Self::JsonPathExtract => "json-path-extract",
            Self::JsonPathUpdate => "json-path-update",
            Self::VectorFlatSearch => "vector-flat-search",
            Self::VectorAnnSearch => "vector-ann-search",
            Self::VectorAnnSearchDisk => "vector-ann-search-disk",
            Self::CommitStormBatched => "commit-storm-batched",
            Self::SecondaryIndexCount => "secondary-index-count",
            Self::SecondaryIndexOrderedLimit => "secondary-index-ordered-limit",
            Self::CoveredRangeCold => "covered-range-cold",
            Self::CoveredRangeWarm => "covered-range-warm",
            Self::HotCounterUpdate => "hot-counter-update",
            Self::QueueMixed => "queue-mixed",
            Self::ChaosLockConvoy => "chaos-lock-convoy",
            Self::ChaosConnectionChurn => "chaos-connection-churn",
            Self::ChaosCheckpointThrash => "chaos-checkpoint-thrash",
            Self::ChaosIndexHammer => "chaos-index-hammer",
            Self::ChaosSortSpillConvoy => "chaos-sort-spill-convoy",
            Self::ChaosSchemaStorm => "chaos-schema-storm",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum DurabilityKind {
    Strict,
    Normal,
    Unsafe,
}

impl DurabilityKind {
    // dedup-allowed: enum-discriminator-method (each arm names a
    // distinct variant of the enclosing enum; collapsing into a
    // shared helper would require boxing both enums under a trait
    // for a 6-line method with zero shared logic).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Normal => "normal",
            Self::Unsafe => "unsafe",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryScenarioKind {
    Wal,
    Catalog,
    Checkpoint,
}

impl RecoveryScenarioKind {
    // dedup-allowed: enum-discriminator-method (see DurabilityKind::as_str).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wal => "wal",
            Self::Catalog => "catalog",
            Self::Checkpoint => "checkpoint",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub engine: EngineKind,
    pub workload: WorkloadKind,
    pub durability: DurabilityKind,
    pub threads: usize,
    pub rows: usize,
    pub duration: Duration,
    pub cache_bytes: usize,
    pub seed: u64,
    pub base_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompareConfig {
    pub out_dir: PathBuf,
    #[serde(default = "default_engines")]
    pub engines: Vec<EngineKind>,
    pub workloads: Vec<WorkloadKind>,
    pub durabilities: Vec<DurabilityKind>,
    pub threads: Vec<usize>,
    #[serde(default = "default_rows")]
    pub rows: usize,
    #[serde(default = "default_seconds")]
    pub seconds: u64,
    #[serde(default = "default_cache_mib")]
    pub cache_mib: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecoveryMatrixConfig {
    #[serde(default = "default_recovery_durabilities")]
    pub durabilities: Vec<DurabilityKind>,
    pub cases: Vec<RecoveryMatrixCase>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecoveryMatrixCase {
    pub name: String,
    pub scenario: RecoveryScenarioKind,
    #[serde(default = "default_rows")]
    pub rows: usize,
    #[serde(default = "default_recovery_kill_windows")]
    pub kill_windows_ms: Vec<u64>,
    #[serde(default = "default_checkpoint_every_rows")]
    pub checkpoint_every_rows: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailpointMatrixConfig {
    #[serde(default = "default_failpoint_durabilities")]
    pub durabilities: Vec<DurabilityKind>,
    pub cases: Vec<FailpointMatrixCase>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailpointMatrixCase {
    pub name: String,
    /// Failpoint name registered in the kernel (e.g.
    /// `engine::commit::before_publish`).
    pub failpoint: String,
    /// Any literal `fail` crate action string: `panic`, `return`,
    /// `return(value)`, `off`, `print(msg)`, `pause`, `sleep(N)`,
    /// `yield`, plus optional `K%` (frequency) and `K*` (count)
    /// prefixes. The kernel `failpoints::cfg` wrapper validates this
    /// against the fail grammar; unknown tokens (e.g. `abort`) are
    /// rejected loudly instead of silently turning into a no-op.
    pub action: String,
    #[serde(default = "default_failpoint_durabilities")]
    pub durabilities: Vec<DurabilityKind>,
    #[serde(default = "default_rows")]
    pub rows: usize,
    #[serde(default = "default_failpoint_kill_after_n_hits")]
    pub kill_after_n_hits: Vec<u64>,
    /// Whether the case is allowed to report `acked == 0`. Default is
    /// false: a case with zero acknowledged commits is suspicious
    /// because the gate-oracle is then vacuous (zero ack rows
    /// trivially recover). Cases that legitimately expect zero acks
    /// (e.g. failpoints that fire before the very first commit's ack
    /// row is written) opt in by setting this to `true`.
    #[serde(default)]
    pub expect_zero_acks: bool,
    /// Expected exit-status class for the child process. The runner
    /// gates on this so a misconfigured action that causes the child
    /// to exit cleanly (instead of dying as expected) flips the case
    /// to `passed = false`. Defaults to `non-zero` because the
    /// canonical lane-fp scenarios are kill cases.
    #[serde(default)]
    pub expect_child_exit: ExpectExit,
}

/// Expected child-process exit-status class for a failpoint matrix
/// case.
///
/// - `NonZero` — the failpoint kills the child via panic / abort and
///   the parent must observe a non-zero exit code or signal-death.
/// - `Zero` — the failpoint short-circuits via `return` and the child
///   returns from the workload cleanly. The `wal-fsync-skipped` case
///   is the canonical example: `return` makes `wal::flush` skip the
///   fsync but the workload otherwise completes normally.
/// - `Any` — opt-out for pre-phase11 or in-flight experimental cases.
///   Avoid in new cases; the whole point of lane-fp is to remove the
///   trivial-pass behaviour that this opt-out preserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectExit {
    #[default]
    NonZero,
    Zero,
    Any,
}

impl CompareConfig {
    // dedup-allowed: per-type TOML config loader. The body is the
    // canonical `read_to_string` + `toml::from_str::<Self>` pair plus a
    // type-specific post-condition check; extracting the IO would
    // strip the `Self`-bound deserialize that distinguishes the two
    // configs and reduce, not improve, clarity.
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("read compare config {}", path.display()))?;
        let config = toml::from_str::<Self>(&raw)
            .with_context(|| format!("parse compare config {}", path.display()))?;
        if config.workloads.is_empty()
            || config.durabilities.is_empty()
            || config.threads.is_empty()
        {
            bail!("compare config must define non-empty workloads, durabilities, and threads");
        }
        Ok(config)
    }

    pub fn run_spec(
        &self,
        engine: &EngineKind,
        workload: &WorkloadKind,
        durability: &DurabilityKind,
        threads: usize,
        seed: u64,
    ) -> Result<RunSpec> {
        Ok(RunSpec {
            engine: *engine,
            workload: *workload,
            durability: *durability,
            threads: threads.max(1),
            rows: self.rows.max(1),
            duration: Duration::from_secs(self.seconds.max(1)),
            cache_bytes: self.cache_mib.max(1) * 1024 * 1024,
            seed,
            base_dir: self.out_dir.join("dbs"),
        })
    }
}

fn default_engines() -> Vec<EngineKind> {
    vec![EngineKind::Redline, EngineKind::Sqlite]
}

fn default_rows() -> usize {
    1024
}

fn default_seconds() -> u64 {
    2
}

fn default_cache_mib() -> usize {
    16
}

fn default_recovery_durabilities() -> Vec<DurabilityKind> {
    vec![DurabilityKind::Strict, DurabilityKind::Normal]
}

fn default_recovery_kill_windows() -> Vec<u64> {
    vec![150, 350, 700]
}

fn default_checkpoint_every_rows() -> usize {
    32
}

fn default_failpoint_durabilities() -> Vec<DurabilityKind> {
    vec![DurabilityKind::Strict]
}

fn default_failpoint_kill_after_n_hits() -> Vec<u64> {
    vec![1]
}

impl FailpointMatrixConfig {
    // dedup-allowed: per-type TOML config loader (see CompareConfig::load).
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("read failpoint matrix {}", path.display()))?;
        let config = toml::from_str::<Self>(&raw)
            .with_context(|| format!("parse failpoint matrix {}", path.display()))?;
        if config.cases.is_empty() {
            bail!("failpoint matrix must define at least one case");
        }
        Ok(config)
    }
}

impl FromStr for WorkloadKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "single-row-insert" => Ok(Self::SingleRowInsert),
            "batched-insert100" | "batched-insert-100" => Ok(Self::BatchedInsert100),
            "point-read-pk" => Ok(Self::PointReadPk),
            "secondary-index-read" => Ok(Self::SecondaryIndexRead),
            "secondary-index-range" => Ok(Self::SecondaryIndexRange),
            "writers-disjoint" => Ok(Self::WritersDisjoint),
            "hot-row-update" => Ok(Self::HotRowUpdate),
            "mixed-oltp" => Ok(Self::MixedOltp),
            "mixed95-read5-write" => Ok(Self::Mixed95Read5Write),
            "mixed80-read20-write" => Ok(Self::Mixed80Read20Write),
            "mixed50-read50-write" => Ok(Self::Mixed50Read50Write),
            "connection-limit" => Ok(Self::ConnectionLimit),
            "large-sort-spill" => Ok(Self::LargeSortSpill),
            "json-path-extract" => Ok(Self::JsonPathExtract),
            "json-path-update" => Ok(Self::JsonPathUpdate),
            "vector-flat-search" => Ok(Self::VectorFlatSearch),
            "vector-ann-search" => Ok(Self::VectorAnnSearch),
            "vector-ann-search-disk" => Ok(Self::VectorAnnSearchDisk),
            "commit-storm-batched" => Ok(Self::CommitStormBatched),
            "secondary-index-count" => Ok(Self::SecondaryIndexCount),
            "secondary-index-ordered-limit" => Ok(Self::SecondaryIndexOrderedLimit),
            "covered-range-cold" => Ok(Self::CoveredRangeCold),
            "covered-range-warm" => Ok(Self::CoveredRangeWarm),
            "hot-counter-update" => Ok(Self::HotCounterUpdate),
            "queue-mixed" => Ok(Self::QueueMixed),
            "chaos-lock-convoy" => Ok(Self::ChaosLockConvoy),
            "chaos-connection-churn" => Ok(Self::ChaosConnectionChurn),
            "chaos-checkpoint-thrash" => Ok(Self::ChaosCheckpointThrash),
            "chaos-index-hammer" => Ok(Self::ChaosIndexHammer),
            "chaos-sort-spill-convoy" => Ok(Self::ChaosSortSpillConvoy),
            "chaos-schema-storm" => Ok(Self::ChaosSchemaStorm),
            _ => bail!("unknown workload {value}"),
        }
    }
}
