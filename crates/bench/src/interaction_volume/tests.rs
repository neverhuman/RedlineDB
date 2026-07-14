use super::*;
use crate::report::LatencySummary;
use tempfile::tempdir;

fn config() -> CertConfig {
    CertConfig {
        threads: vec![1, 4],
        operations_per_thread: 100,
        repetitions: 1,
        sessions: 8,
        payload_bytes: 32,
        warmup_operations_per_thread: 4,
        seed: 7,
        durability: "strict".to_owned(),
        idle_observation_secs: 1,
        soak_observation_secs: 1,
        max_idle_growth_bytes: 1024,
        max_data_bytes: 1024 * 1024,
    }
}

fn canonical_config() -> CertConfig {
    CertConfig {
        threads: CANONICAL_THREADS.to_vec(),
        operations_per_thread: CANONICAL_OPERATIONS_PER_THREAD,
        repetitions: CANONICAL_REPETITIONS,
        sessions: CANONICAL_SESSIONS,
        payload_bytes: CANONICAL_PAYLOAD_BYTES,
        warmup_operations_per_thread: CANONICAL_WARMUP_OPERATIONS_PER_THREAD,
        seed: 7,
        durability: "strict".to_owned(),
        idle_observation_secs: CANONICAL_IDLE_OBSERVATION_SECS,
        soak_observation_secs: CANONICAL_SOAK_OBSERVATION_SECS,
        max_idle_growth_bytes: CANONICAL_MAX_IDLE_GROWTH_BYTES,
        max_data_bytes: CANONICAL_MAX_DATA_BYTES,
    }
}

fn args(config: &CertConfig, mode: CertMode) -> InteractionVolumeArgs {
    InteractionVolumeArgs {
        out_dir: PathBuf::from("unused-test-output"),
        mode,
        approved_profile: None,
        execution_evidence: PathBuf::from("unused-execution-evidence.json"),
        deadline_secs: 300,
        deadline_unix_ms: None,
        threads: config.threads.clone(),
        operations_per_thread: config.operations_per_thread,
        repetitions: config.repetitions,
        sessions: config.sessions,
        payload_bytes: config.payload_bytes,
        warmup_operations_per_thread: config.warmup_operations_per_thread,
        seed: config.seed,
        idle_observation_secs: config.idle_observation_secs,
        soak_observation_secs: config.soak_observation_secs,
        max_idle_growth_bytes: config.max_idle_growth_bytes,
        max_data_bytes: config.max_data_bytes,
    }
}

#[test]
fn operation_plan_is_deterministic_and_has_the_exact_requested_shape() {
    let config = config();
    let first = interaction_plan(&config, 4);
    let second = interaction_plan(&config, 4);
    assert_eq!(sha256_json(&first).unwrap(), sha256_json(&second).unwrap());
    assert_eq!(first.len(), 4);
    assert!(first.iter().all(|worker| worker.len() == 100));
    assert!(
        first
            .iter()
            .flatten()
            .any(|op| matches!(op, Interaction::Append { .. }))
    );
    assert!(
        first
            .iter()
            .flatten()
            .any(|op| matches!(op, Interaction::ReplayEvents { .. }))
    );
}

#[test]
fn release_profile_is_exact_and_thresholds_cannot_be_relaxed() {
    let canonical = canonical_config();
    assert!(is_canonical_profile(&canonical));

    let mut relaxed_storage = canonical.clone();
    relaxed_storage.max_data_bytes += 1;
    assert!(!is_canonical_profile(&relaxed_storage));

    let mut shorter_soak = canonical;
    shorter_soak.soak_observation_secs -= 1;
    assert!(!is_canonical_profile(&shorter_soak));

    let mut changed_seed = canonical_config();
    changed_seed.seed = 8;
    assert!(!is_canonical_profile(&changed_seed));
}

#[test]
fn checked_in_daily_profile_has_the_literal_approved_digest() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/interaction-volume-daily-v1.json");
    assert_eq!(sha256_file(&path).unwrap(), APPROVED_DAILY_PROFILE_SHA256);
    let document: ApprovedProfileDocument =
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(document.schema_version, APPROVED_PROFILE_SCHEMA_VERSION);
    assert_eq!(document.profile_id, APPROVED_PROFILE_ID);
    assert_eq!(document.config, canonical_config());
    assert!(is_canonical_profile(&document.config));
}

#[test]
fn release_requires_the_exact_approved_profile_and_absolute_ceilings() {
    let profile =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/interaction-volume-daily-v1.json");
    let canonical = canonical_config();
    let mut release = args(&canonical, CertMode::Release);
    assert!(validate_approved_profile(&release, &canonical).is_err());

    release.approved_profile = Some(profile);
    assert_eq!(
        validate_approved_profile(&release, &canonical).unwrap(),
        Some(APPROVED_DAILY_PROFILE_SHA256.to_owned())
    );

    let mut changed_seed = canonical.clone();
    changed_seed.seed += 1;
    assert!(validate_approved_profile(&release, &changed_seed).is_err());

    let mut relaxed_storage = args(&canonical, CertMode::Smoke);
    relaxed_storage.max_data_bytes = ABSOLUTE_MAX_DATA_BYTES + 1;
    assert!(validated_config(&relaxed_storage).is_err());
    let mut relaxed_growth = args(&canonical, CertMode::Smoke);
    relaxed_growth.max_idle_growth_bytes = ABSOLUTE_MAX_IDLE_GROWTH_BYTES + 1;
    assert!(validated_config(&relaxed_growth).is_err());
}

#[test]
fn attempt_plan_covers_every_engine_and_one_delayed_growth_soak() {
    let config = canonical_config();
    let planned = planned_runs(&config).unwrap();
    assert_eq!(planned.len(), config.threads.len() * config.repetitions * 3);
    assert_eq!(
        planned.iter().filter(|run| run.delayed_growth_soak).count(),
        1
    );
    assert!(planned.iter().all(|run| run.plan_sha256.len() == 64));
}

#[test]
fn gitlab_daily_certificate_is_pinned_serialized_and_not_retried() {
    fn job_block<'a>(yaml: &'a str, name: &str) -> &'a str {
        let marker = format!("{name}:");
        let start = yaml
            .lines()
            .enumerate()
            .find(|(_, line)| *line == marker)
            .map(|(index, _)| index)
            .unwrap();
        let lines = yaml.lines().collect::<Vec<_>>();
        let end = lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find(|(_, line)| {
                !line.is_empty() && !line.starts_with(char::is_whitespace) && !line.starts_with('#')
            })
            .map(|(index, _)| index)
            .unwrap_or(lines.len());
        let start_byte = lines[..=start]
            .iter()
            .map(|line| line.len() + 1)
            .sum::<usize>();
        let end_byte = lines[..end]
            .iter()
            .map(|line| line.len() + 1)
            .sum::<usize>();
        &yaml[start_byte..end_byte.min(yaml.len())]
    }

    let yaml = include_str!("../../../../.gitlab-ci.yml");
    let pinned_digest = PINNED_POSTGRES_IMAGE_DIGEST;
    for name in ["benchmark", "benchmark-rql", "interaction-volume-daily"] {
        let block = job_block(yaml, name);
        assert!(block.contains("resource_group: redline-heavy-benchmark"));
        assert!(block.contains("interruptible: false"));
        assert!(block.contains("retry: 0"));
    }
    let smoke = job_block(yaml, "interaction-volume-smoke");
    assert!(smoke.contains(pinned_digest));
    assert!(smoke.contains("alias: postgres-cert"));
    assert!(smoke.contains("interaction-volume-interruption-test.sh"));

    let daily = job_block(yaml, "interaction-volume-daily");
    assert!(!daily.contains("services:"));
    assert!(!daily.contains("REDLINEDB_POSTGRES_CERT_CI_SERVICE"));
    assert!(daily.contains("REDLINEDB_INTERACTION_TRIGGER_CONTRACT"));
    assert!(daily.contains("expire_in: 90 days"));

    let wrapper = include_str!("../../../../ops/ci/interaction-volume-cert.sh");
    assert!(wrapper.contains(pinned_digest));
    assert!(wrapper.contains("type=bind"));
    assert!(!wrapper.contains("--tmpfs /var/lib/postgresql/data"));

    let trigger: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../ops/ci/interaction-volume-trigger-contract.json"
    ))
    .unwrap();
    assert_eq!(trigger["daily_job"], "interaction-volume-daily");
    assert_eq!(trigger["artifact_retention_days"], 90);
}

#[test]
fn execution_order_rotates_every_engine_through_every_position() {
    let orders = (0..3)
        .map(|repetition| engine_order(0, repetition, 7))
        .collect::<Vec<_>>();
    for position in 0..3 {
        let engines = orders
            .iter()
            .map(|order| order[position])
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(engines.len(), 3);
    }
}

#[test]
fn output_directory_is_exclusive_and_existing_receipts_are_immutable() {
    let dir = tempdir().unwrap();
    let first = OutputLock::acquire(dir.path()).unwrap();
    assert!(OutputLock::acquire(dir.path()).is_err());
    drop(first);
    fs::write(dir.path().join("manifest.json"), b"{}\n").unwrap();
    assert!(OutputLock::acquire(dir.path()).is_err());
}

#[test]
fn idle_growth_uses_peak_not_only_the_last_sample() {
    let samples = vec![
        StorageSample {
            offset_ms: 0,
            data_bytes: 100,
            wal_bytes: 10,
        },
        StorageSample {
            offset_ms: 1_000,
            data_bytes: 250,
            wal_bytes: 10,
        },
        StorageSample {
            offset_ms: 2_000,
            data_bytes: 150,
            wal_bytes: 10,
        },
    ];
    assert_eq!(idle_growth(&samples), 150);
}

#[test]
fn atomic_receipt_round_trips_and_has_a_stable_digest() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("receipt.json");
    atomic_write_json(&path, &config()).unwrap();
    let decoded: CertConfig = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(decoded.threads, vec![1, 4]);
    assert_eq!(sha256_file(&path).unwrap().len(), 64);
    assert!(!fs::read_dir(dir.path()).unwrap().flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(".receipt.json.tmp-")
    }));
}

#[test]
fn strict_comparison_gate_requires_both_throughput_and_tail_wins() {
    fn run(engine: EngineLabel, throughput: f64, p99: u64) -> EngineRun {
        let integrity = IntegritySnapshot {
            events: 1,
            sessions: 1,
            last_sequence_sum: 1,
            content_sha256: "a".repeat(64),
            session_state_sha256: "c".repeat(64),
        };
        EngineRun {
            engine,
            engine_version: "test".to_owned(),
            threads: 1,
            repetition: 0,
            execution_order_position: 0,
            plan_sha256: "b".repeat(64),
            attempted_operations: 1,
            metrics: MetricsSummary {
                operations: 1,
                failures: 0,
                busy_errors: 0,
                locked_errors: 0,
                timeout_errors: 0,
                elapsed_ms: 1,
                throughput_ops_per_sec: throughput,
                latency: LatencySummary {
                    p50_us: p99,
                    p95_us: p99,
                    p99_us: p99,
                    p999_us: p99,
                    max_us: p99,
                },
            },
            retry_attempts: 0,
            failure_samples: Vec::new(),
            latency_sample_us: vec![p99],
            point_reads_verified: 0,
            replays_verified: 0,
            replay_rows_verified: 0,
            storage_before: StorageSample {
                offset_ms: 0,
                data_bytes: 1,
                wal_bytes: 1,
            },
            workload_storage_samples: vec![StorageSample {
                offset_ms: 0,
                data_bytes: 1,
                wal_bytes: 1,
            }],
            storage_after_checkpoint: StorageSample {
                offset_ms: 1,
                data_bytes: 1,
                wal_bytes: 1,
            },
            idle_storage_samples: vec![StorageSample {
                offset_ms: 1,
                data_bytes: 1,
                wal_bytes: 1,
            }],
            lifecycle_storage_samples: [
                "setup_complete",
                "workload_complete",
                "checkpoint_complete",
                "idle_observation_complete",
                "before_integrity",
                "integrity_complete",
                "reopen_complete",
                "reopen_integrity_complete",
            ]
            .into_iter()
            .map(|phase| PhaseStorageSample {
                phase: phase.to_owned(),
                sample: StorageSample {
                    offset_ms: 1,
                    data_bytes: 1,
                    wal_bytes: 1,
                },
            })
            .collect(),
            storage_semantics: storage_semantics(engine, 1024 * 1024, true),
            delayed_growth_soak: engine == EngineLabel::Redline,
            idle_growth_bytes: 0,
            expected_integrity: integrity.clone(),
            integrity_before_reopen: integrity.clone(),
            integrity_after_reopen: integrity,
            plan_integrity_verified: true,
            reopen_verified: true,
            engine_stats: serde_json::json!({}),
        }
    }
    let winning = vec![
        run(EngineLabel::Redline, 120.0, 80),
        run(EngineLabel::Sqlite, 100.0, 100),
        run(EngineLabel::Postgres, 110.0, 90),
    ];
    assert!(comparisons(&winning, &[1]).unwrap()[0].bounded_result_eligible);
    let losing = vec![
        run(EngineLabel::Redline, 90.0, 80),
        run(EngineLabel::Sqlite, 100.0, 100),
        run(EngineLabel::Postgres, 110.0, 90),
    ];
    assert!(!comparisons(&losing, &[1]).unwrap()[0].bounded_result_eligible);
}

#[test]
fn redline_and_sqlite_integrity_hash_text_cells_identically() {
    let root = tempdir().unwrap();
    let config = config();
    let operation = Interaction::Append {
        event_id: "event-text-cell".to_owned(),
        session_id: 0,
        sequence: 1,
        payload: "canonical-payload".to_owned(),
    };
    let expected = expected_integrity(&[vec![operation.clone()]], config.sessions).unwrap();
    let mut snapshots = Vec::new();

    for label in [EngineLabel::Redline, EngineLabel::Sqlite] {
        let run_dir = root.path().join(label.as_str());
        fs::create_dir_all(&run_dir).unwrap();
        let spec = run_spec(label, &config, 1, &run_dir);
        let engine: Box<dyn BenchEngine> = match label {
            EngineLabel::Redline => Box::new(RedlineEngine::open(&spec, &run_dir).unwrap()),
            EngineLabel::Sqlite => Box::new(SqliteEngine::open(&spec, &run_dir).unwrap()),
            EngineLabel::Postgres => unreachable!(),
        };
        setup_interaction_schema(&*engine, config.sessions).unwrap();
        let mut connection = engine.connect(0).unwrap();
        let validation = ResultValidation::for_plan(&[vec![operation.clone()]], &config).unwrap();
        execute_interaction(&mut *connection, &operation, &validation).unwrap();
        drop(connection);
        snapshots.push(interaction_integrity(&*engine).unwrap());
    }

    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0], snapshots[1]);
    assert_eq!(snapshots[0], expected);
    assert_eq!(snapshots[0].events, 1);
    assert_eq!(snapshots[0].content_sha256.len(), 64);
}

struct QueryOnlyConn {
    row: Vec<CellValue>,
    rows: Vec<Vec<CellValue>>,
}

impl BenchConn for QueryOnlyConn {
    fn execute(&mut self, _sql: &str, _params: &[CellValue]) -> Result<u64> {
        Ok(0)
    }

    fn query_row(&mut self, _sql: &str, _params: &[CellValue]) -> Result<Vec<CellValue>> {
        Ok(self.row.clone())
    }

    fn query_all(&mut self, _sql: &str, _params: &[CellValue]) -> Result<Vec<Vec<CellValue>>> {
        Ok(self.rows.clone())
    }

    fn begin_immediate(&mut self) -> Result<()> {
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        Ok(())
    }

    fn set_busy_timeout(&mut self, _timeout: Duration) -> Result<()> {
        Ok(())
    }
}

#[test]
fn every_timed_read_and_replay_is_content_checked() {
    let config = config();
    let append = Interaction::Append {
        event_id: "w000-o00000000".to_owned(),
        session_id: 0,
        sequence: 0,
        payload: "a".repeat(config.payload_bytes),
    };
    let read = Interaction::ReadSession { session_id: 0 };
    let replay = Interaction::ReplayEvents { session_id: 0 };
    let validation =
        ResultValidation::for_plan(&[vec![append, read.clone(), replay.clone()]], &config).unwrap();

    let mut valid_read = QueryOnlyConn {
        row: vec![
            CellValue::Integer(0),
            CellValue::Integer(1),
            CellValue::Text("training".to_owned()),
        ],
        rows: Vec::new(),
    };
    assert_eq!(
        execute_interaction(&mut valid_read, &read, &validation)
            .unwrap()
            .point_reads,
        1
    );
    valid_read.row[2] = CellValue::Text("corrupt".to_owned());
    assert!(execute_interaction(&mut valid_read, &read, &validation).is_err());

    let valid_replay_row = vec![
        CellValue::Text("w000-o00000000".to_owned()),
        CellValue::Integer(0),
        CellValue::Integer(0),
        CellValue::Text("training.progress".to_owned()),
        CellValue::Text("a".repeat(config.payload_bytes)),
    ];
    let mut valid_replay = QueryOnlyConn {
        row: Vec::new(),
        rows: vec![valid_replay_row.clone()],
    };
    let verified = execute_interaction(&mut valid_replay, &replay, &validation).unwrap();
    assert_eq!((verified.replays, verified.replay_rows), (1, 1));
    valid_replay.rows[0][2] = CellValue::Integer(99);
    assert!(execute_interaction(&mut valid_replay, &replay, &validation).is_err());
}

#[test]
fn storage_watchdog_stops_with_headroom_before_the_hard_cap() {
    let hard_limit = 64 * 1024 * 1024;
    let stop = storage_stop_threshold(hard_limit);
    assert!(stop < hard_limit);
    assert!(
        ensure_storage_within_limit(
            &StorageSample {
                offset_ms: 0,
                data_bytes: stop - 1,
                wal_bytes: 0,
            },
            hard_limit,
        )
        .is_ok()
    );
    assert!(
        ensure_storage_within_limit(
            &StorageSample {
                offset_ms: 0,
                data_bytes: stop,
                wal_bytes: 0,
            },
            hard_limit,
        )
        .is_err()
    );
}

#[test]
fn progress_receipt_names_active_point_phase_deadline_and_cause() {
    let dir = tempdir().unwrap();
    ProgressTracker::matrix(dir.path(), 3, &[], unix_millis() + 60_000)
        .active(ProgressPoint {
            engine: EngineLabel::Postgres,
            threads: 4,
            repetition: 2,
            execution_order_position: 1,
        })
        .write(
            "failed",
            "timed_workload",
            Some("test cause".to_owned()),
            Some(StorageSample {
                offset_ms: 10,
                data_bytes: 20,
                wal_bytes: 30,
            }),
        )
        .unwrap();
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("progress.json")).unwrap()).unwrap();
    assert_eq!(receipt["active_engine"], "postgres");
    assert_eq!(receipt["active_point"]["threads"], 4);
    assert_eq!(receipt["lifecycle_phase"], "timed_workload");
    assert_eq!(receipt["cause"], "test cause");
    assert!(receipt["heartbeat_unix_ms"].as_u64().unwrap() > 0);
    assert!(receipt["deadline_unix_ms"].as_u64().unwrap() > 0);
}

#[test]
fn execution_evidence_binds_git_binary_postgres_and_shared_mount() {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let commit = String::from_utf8(commit.stdout).unwrap().trim().to_owned();
    let dirty = !std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .unwrap()
        .stdout
        .is_empty();
    let document = ExecutionEvidence {
        schema_version: EXECUTION_EVIDENCE_SCHEMA.to_owned(),
        generator: "ops/ci/interaction-volume-cert.sh".to_owned(),
        source_commit: commit,
        source_dirty: dirty,
        binary_sha256: "b".repeat(64),
        postgres: PostgresExecutionEvidence {
            image_digest: PINNED_POSTGRES_IMAGE_DIGEST.to_owned(),
            digest_observation: "docker_image_inspect_repo_digest".to_owned(),
            digest_verified: true,
            backend: "docker_bind".to_owned(),
            isolation_verified: true,
            dedicated_instance: true,
            endpoint_scope: "loopback".to_owned(),
        },
        storage: StorageContract {
            class: "shared_host_durable_bind".to_owned(),
            local_database_root: "/controlled/local".to_owned(),
            postgres_data_root: "/controlled/postgres".to_owned(),
            local_mount_identity: "device=1;fstype=ext4".to_owned(),
            postgres_mount_identity: "device=1;fstype=ext4".to_owned(),
            same_mount: true,
            durable: true,
            postgres_bind_mode: "rw".to_owned(),
        },
    };
    let dir = tempdir().unwrap();
    let path = dir.path().join("evidence.json");
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let validated = load_execution_evidence(&path, &"b".repeat(64)).unwrap();
    assert!(validated.source_observation_bound);
    assert!(validated.postgres_provenance_bound);
    assert!(validated.storage_comparison_eligible);
    assert!(load_execution_evidence(&path, &"c".repeat(64)).is_err());
}

#[test]
fn service_or_memory_storage_can_never_authorize_a_reference_win() {
    let postgres = PostgresExecutionEvidence {
        image_digest: PINNED_POSTGRES_IMAGE_DIGEST.to_owned(),
        digest_observation: "ci_service_contract".to_owned(),
        digest_verified: false,
        backend: "ci_service".to_owned(),
        isolation_verified: true,
        dedicated_instance: true,
        endpoint_scope: "ci_service".to_owned(),
    };
    let storage = StorageContract {
        class: "unmatched_ci_service".to_owned(),
        local_database_root: "/local".to_owned(),
        postgres_data_root: "ci-service:postgres-cert".to_owned(),
        local_mount_identity: "device=1;fstype=tmpfs".to_owned(),
        postgres_mount_identity: "unobservable".to_owned(),
        same_mount: false,
        durable: false,
        postgres_bind_mode: "service_managed".to_owned(),
    };
    assert!(!storage.comparison_eligible(&postgres));
}
