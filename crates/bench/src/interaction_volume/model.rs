use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineLabel {
    Redline,
    Sqlite,
    Postgres,
}

impl EngineLabel {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Redline => "redline",
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum Interaction {
    Append {
        event_id: String,
        session_id: i64,
        sequence: i64,
        payload: String,
    },
    ReadSession {
        session_id: i64,
    },
    ReplayEvents {
        session_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertConfig {
    pub threads: Vec<usize>,
    pub operations_per_thread: usize,
    pub repetitions: usize,
    pub sessions: usize,
    pub payload_bytes: usize,
    pub warmup_operations_per_thread: usize,
    pub seed: u64,
    pub durability: String,
    pub idle_observation_secs: u64,
    pub soak_observation_secs: u64,
    pub max_idle_growth_bytes: u64,
    pub max_data_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSample {
    pub offset_ms: u64,
    pub data_bytes: u64,
    pub wal_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStorageSample {
    pub phase: String,
    pub sample: StorageSample,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSemantics {
    pub accounting: String,
    pub purpose: String,
    pub cross_engine_comparable: bool,
    pub sampled_inside_timed_window: bool,
    pub continuously_sampled: bool,
    pub hard_limit_bytes: u64,
    pub stop_threshold_bytes: u64,
    pub shared_storage_contract: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegritySnapshot {
    pub events: u64,
    pub sessions: u64,
    pub last_sequence_sum: i64,
    pub content_sha256: String,
    pub session_state_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineRun {
    pub engine: EngineLabel,
    pub engine_version: String,
    pub threads: usize,
    pub repetition: usize,
    pub execution_order_position: usize,
    pub plan_sha256: String,
    pub attempted_operations: u64,
    pub metrics: MetricsSummary,
    pub retry_attempts: u64,
    pub failure_samples: Vec<String>,
    pub point_reads_verified: u64,
    pub replays_verified: u64,
    pub replay_rows_verified: u64,
    /// Deterministic bounded sample (at most 4096 measurements) in worker/operation order.
    pub latency_sample_us: Vec<u64>,
    pub storage_before: StorageSample,
    pub workload_storage_samples: Vec<StorageSample>,
    pub storage_after_checkpoint: StorageSample,
    pub idle_storage_samples: Vec<StorageSample>,
    pub lifecycle_storage_samples: Vec<PhaseStorageSample>,
    pub storage_semantics: StorageSemantics,
    pub delayed_growth_soak: bool,
    pub idle_growth_bytes: u64,
    pub expected_integrity: IntegritySnapshot,
    pub integrity_before_reopen: IntegritySnapshot,
    pub integrity_after_reopen: IntegritySnapshot,
    pub plan_integrity_verified: bool,
    pub reopen_verified: bool,
    pub engine_stats: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub threads: usize,
    pub redline_median_ops_per_sec: f64,
    pub sqlite_median_ops_per_sec: f64,
    pub postgres_median_ops_per_sec: f64,
    pub redline_to_sqlite_throughput_ratio: f64,
    pub redline_to_postgres_throughput_ratio: f64,
    pub redline_median_p99_us: u64,
    pub sqlite_median_p99_us: u64,
    pub postgres_median_p99_us: u64,
    pub redline_to_sqlite_p99_ratio: f64,
    pub redline_to_postgres_p99_ratio: f64,
    pub worst_redline_to_sqlite_throughput_ratio: f64,
    pub worst_redline_to_postgres_throughput_ratio: f64,
    pub worst_redline_to_sqlite_p99_ratio: f64,
    pub worst_redline_to_postgres_p99_ratio: f64,
    /// Exact-point bounded result only; every repetition must beat both references on both axes.
    pub bounded_result_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawReceipt {
    pub schema_version: String,
    pub environment: RunEnvironment,
    pub execution_evidence_sha256: String,
    pub storage_contract: StorageContract,
    pub config: CertConfig,
    pub runs: Vec<EngineRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ApprovedProfileDocument {
    pub schema_version: String,
    pub profile_id: String,
    pub approval_policy: String,
    pub config: CertConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PlannedRun {
    pub engine: EngineLabel,
    pub threads: usize,
    pub repetition: usize,
    pub execution_order_position: usize,
    pub plan_sha256: String,
    pub delayed_growth_soak: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertManifest {
    pub schema_version: String,
    pub mode: CertMode,
    pub status: String,
    pub mechanics_passed: bool,
    pub release_eligible: bool,
    pub bounded_reference_win_eligible: bool,
    pub claim_scope: String,
    pub canonical_profile: bool,
    pub approved_profile_sha256: Option<String>,
    pub postgres_image_digest: String,
    pub execution_evidence: ExecutionEvidence,
    pub execution_evidence_sha256: String,
    pub provenance_bound: bool,
    pub storage_contract: StorageContract,
    pub storage_comparison_eligible: bool,
    pub config_sha256: String,
    pub artifact_sha256: String,
    pub attempt_receipt: String,
    pub attempt_receipt_sha256: String,
    pub failure_reasons: Vec<String>,
    pub environment: RunEnvironment,
    pub config: CertConfig,
    pub raw_receipt: String,
    pub raw_receipt_sha256: String,
    pub comparisons: Vec<Comparison>,
    pub tested_workload: Option<TestedWorkload>,
    pub reference_cleanup_verified: bool,
    pub storage_claim_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestedWorkload {
    pub exact_concurrent_worker_points: Vec<usize>,
    pub operations_per_worker: usize,
    pub total_operations_at_each_point: Vec<u64>,
    pub sessions: usize,
    pub payload_bytes: usize,
    pub seed: u64,
    pub append_percent: u8,
    pub point_read_percent: u8,
    pub replay_percent: u8,
    pub max_observed_data_bytes: u64,
    pub storage_stop_threshold_bytes: u64,
    pub storage_class: String,
    pub durability: String,
    pub delayed_growth_soak_secs: u64,
}

pub(super) struct WorkerResult {
    pub metrics: Metrics,
    pub retry_attempts: u64,
    pub latency_sample_us: Vec<u64>,
    pub failure_samples: Vec<String>,
    pub verification: InteractionVerification,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct InteractionVerification {
    pub point_reads: u64,
    pub replays: u64,
    pub replay_rows: u64,
}

pub(super) struct ResultValidation {
    max_appends_by_session: Vec<u64>,
    expected_events: std::collections::BTreeMap<String, (i64, i64, String)>,
    pub operations_per_thread: usize,
}

impl ResultValidation {
    pub(super) fn for_plan(plan: &[Vec<Interaction>], config: &CertConfig) -> Result<Self> {
        let mut max_appends_by_session = vec![0_u64; config.sessions];
        let mut expected_events = std::collections::BTreeMap::new();
        for interaction in plan.iter().flatten() {
            if let Interaction::Append {
                event_id,
                session_id,
                sequence,
                payload,
            } = interaction
            {
                let index = usize::try_from(*session_id)
                    .ok()
                    .filter(|index| *index < config.sessions)
                    .context("interaction plan contains an invalid session")?;
                max_appends_by_session[index] = max_appends_by_session[index].saturating_add(1);
                if expected_events
                    .insert(event_id.clone(), (*session_id, *sequence, payload.clone()))
                    .is_some()
                {
                    bail!("interaction plan contains duplicate event id {event_id}");
                }
            }
        }
        Ok(Self {
            max_appends_by_session,
            expected_events,
            operations_per_thread: config.operations_per_thread,
        })
    }

    pub(super) fn max_appends(&self, session_id: i64) -> Result<u64> {
        usize::try_from(session_id)
            .ok()
            .and_then(|index| self.max_appends_by_session.get(index).copied())
            .context("interaction references a session outside the validation plan")
    }

    pub(super) fn expected_event(&self, event_id: &str) -> Result<&(i64, i64, String)> {
        self.expected_events
            .get(event_id)
            .with_context(|| format!("replay returned event outside immutable plan: {event_id}"))
    }
}

pub(super) struct WorkerCompletion {
    pub completed: Arc<AtomicUsize>,
}

impl Drop for WorkerCompletion {
    fn drop(&mut self) {
        self.completed.fetch_add(1, Ordering::Release);
    }
}

pub(super) struct OutputLock {
    path: PathBuf,
}

impl OutputLock {
    pub(super) fn acquire(out_dir: &Path) -> Result<Self> {
        fs::create_dir_all(out_dir)?;
        for receipt in [
            "attempt.json",
            "progress.json",
            "raw-runs.json",
            "manifest.json",
        ] {
            if out_dir.join(receipt).exists() {
                bail!(
                    "refusing to overwrite existing certification receipt {}",
                    out_dir.join(receipt).display()
                );
            }
        }
        let path = out_dir.join(".interaction-volume.lock");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| format!("exclusive certification lock {}", path.display()))?;
        writeln!(file, "pid={}", std::process::id())?;
        file.sync_all()?;
        Ok(Self { path })
    }
}

impl Drop for OutputLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(super) struct RunCleanup<'a> {
    pub label: EngineLabel,
    pub run_dir: &'a Path,
    pub postgres_url: &'a str,
    pub armed: bool,
}

impl RunCleanup<'_> {
    pub(super) fn cleanup(&mut self) -> Result<()> {
        if !self.armed {
            return Ok(());
        }
        if self.label == EngineLabel::Postgres {
            PostgresEngine::cleanup_for_path(self.postgres_url, self.run_dir)?;
        }
        if self.run_dir.exists() {
            fs::remove_dir_all(self.run_dir)?;
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for RunCleanup<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
