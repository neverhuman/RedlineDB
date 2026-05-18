//! Lane E failpoint matrix runner.
//!
//! The runner spawns a child process per (engine, durability, case,
//! kill_after_n_hits) tuple. Each child arms one of the kernel
//! failpoints registered in `crates/kernel/src/{wal,engine,index,
//! catalog,storage}` with an action that kills the process the moment
//! the site fires (typically `panic`).
//!
//! Inside the child the workload follows the strict fsynced-ack
//! protocol used by the recovery matrix: for every row the child
//! issues `INSERT ... COMMIT` and only after the commit succeeds does
//! it append `key\n` to `ack_log` and `flush()` the file. The kernel
//! has already fsynced both the WAL record and the ack-log line by the
//! time we proceed to the next row. When the parent observes the
//! child has died, the highest line in `ack_log` is the contractual
//! upper bound on rows the engine guaranteed durable.
//!
//! The parent then opens a fresh `redlinedb::Database` over the same
//! directory and counts surviving rows. If `recovered < acked`, that
//! is a lost-acked-commit and the case fails the strict gate.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{
    DurabilityKind, EngineKind, ExpectExit, FailpointChildArgs, FailpointMatrixArgs,
    FailpointMatrixCase, FailpointMatrixConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailpointMatrixReport {
    pub seed: u64,
    pub passed: bool,
    pub failed_cases: usize,
    pub runs: Vec<FailpointMatrixRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailpointMatrixRun {
    pub case: String,
    pub failpoint: String,
    pub action: String,
    pub engine: EngineKind,
    pub durability: DurabilityKind,
    pub kill_after_n_hits: u64,
    pub child_status: String,
    /// Numeric exit code captured from the child's `Command::status`.
    /// `None` when the child died from an unhandled signal (e.g.
    /// SIGABRT from the panic-hook abort) and the OS therefore did
    /// not produce an exit code. The gate uses this together with the
    /// case's `expect_child_exit` to decide whether the death was the
    /// expected one.
    #[serde(default)]
    pub child_exit_status: Option<i32>,
    pub acknowledged: usize,
    pub recovered: usize,
    pub lost_acked_commits: i64,
    pub passed: bool,
    /// Plain-English explanation of why the case passed or failed.
    /// Surfaced in the per-case JSON so a falling case is debuggable
    /// from the report alone, without reproducing the run.
    pub pass_reason: String,
}

pub fn run(args: &FailpointMatrixArgs) -> Result<FailpointMatrixReport> {
    let config = FailpointMatrixConfig::load(&args.config)?;
    let mut runs = Vec::new();
    let mut failed = 0_usize;

    // For now only redline supports the failpoint-matrix runner. Sqlite
    // has no equivalent scriptable injection point; the gate explicitly
    // targets the redline engine under strict durability.
    let engine = EngineKind::Redline;

    let total_planned: usize = config
        .cases
        .iter()
        .map(|case| {
            let durabilities = if case.durabilities.is_empty() {
                config.durabilities.len()
            } else {
                case.durabilities.len()
            };
            let kills = case.kill_after_n_hits.len().max(1);
            durabilities * kills
        })
        .sum();
    eprintln!(
        "failpoint-matrix: planning {total_planned} runs across {} cases",
        config.cases.len()
    );

    let mut completed = 0_usize;
    for case in &config.cases {
        let durabilities = if case.durabilities.is_empty() {
            config.durabilities.clone()
        } else {
            case.durabilities.clone()
        };
        let kill_hits = if case.kill_after_n_hits.is_empty() {
            vec![1]
        } else {
            case.kill_after_n_hits.clone()
        };
        for &durability in &durabilities {
            for &kill_after_n_hits in &kill_hits {
                eprintln!(
                    "failpoint-matrix: [{}/{}] case={} durability={} kill_after_n_hits={}",
                    completed + 1,
                    total_planned,
                    case.name,
                    durability.as_str(),
                    kill_after_n_hits
                );
                let run = run_case(engine, durability, case, kill_after_n_hits)?;
                if !run.passed {
                    failed += 1;
                }
                eprintln!(
                    "failpoint-matrix:   -> {} acked={} recovered={} lost={} exit={:?} reason={}",
                    if run.passed { "PASS" } else { "FAIL" },
                    run.acknowledged,
                    run.recovered,
                    run.lost_acked_commits,
                    run.child_exit_status,
                    run.pass_reason,
                );
                runs.push(run);
                completed += 1;
            }
        }
    }

    let passed = failed == 0;
    let report = FailpointMatrixReport {
        seed: args.seed,
        passed,
        failed_cases: failed,
        runs,
    };
    // Cross-check the strict gate via the same evaluator the gates
    // module uses; if the runner ever forgets to flip `passed`, the
    // gate-side audit will still surface a true/false discrepancy.
    let gate = crate::gates::gate_zero_lost_acked_commits(&report);
    if !gate.passed && report.passed {
        // Internal consistency error - fail loudly.
        anyhow::bail!(
            "failpoint matrix runner reported pass but strict gate disagreed: {}",
            gate.detail
        );
    }
    Ok(report)
}

pub fn write_report(out: &Path, report: &FailpointMatrixReport) -> Result<()> {
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir for {}", out.display()))?;
    }
    let mut file = File::create(out)
        .with_context(|| format!("create failpoint-matrix report {}", out.display()))?;
    let body = serde_json::to_string_pretty(report)?;
    file.write_all(body.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

pub fn run_child(args: &FailpointChildArgs) -> Result<()> {
    fs::create_dir_all(&args.db_dir)?;
    if let Some(parent) = args.ack_log.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    // Failpoints commonly fire on background threads (the WAL writer
    // thread, for instance). A Rust panic on a background thread
    // unwinds that thread but does not kill the process; subsequent
    // foreground operations would block forever waiting on the dead
    // worker. Install an abort-on-panic hook so any failpoint panic
    // anywhere in the child terminates the whole process and lets the
    // parent observe a clean death.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        original_hook(info);
        std::process::abort();
    }));

    // Initialise the registry BEFORE we open the engine — `init` is
    // idempotent and only succeeds when the `failpoints` feature is
    // compiled in. The actual `cfg` call is deferred until after the
    // schema is up so the schema-setup commits are NOT counted
    // against `kill_after_n_hits`. Previously the failpoint was armed
    // before `CREATE TABLE`, which meant `engine::commit::before_publish`
    // (and any other commit-path hook) fired during schema creation
    // and the workload never reached the first INSERT — making
    // `acked = 0` for every kill case.
    redlinedb_kernel::failpoints::init();

    let mut options = redlinedb::OpenOptions::default();
    options.memory.cache_bytes = 8 * 1024 * 1024;
    options.durability = match args.durability {
        DurabilityKind::Strict => redlinedb::Durability::Strict,
        DurabilityKind::Normal => redlinedb::Durability::Normal,
        DurabilityKind::Unsafe => redlinedb::Durability::UnsafeDev,
    };
    let db_path = args.db_dir.join("bench.redline");
    let db = redlinedb::Database::open_with_options(&db_path, options)?;
    let mut conn = db.connect()?;

    // Setup the schema. CREATE INDEX is included so that catalog and
    // index failpoints have a non-trivial schema to operate on.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS kv(k INTEGER PRIMARY KEY, tenant INTEGER, v BLOB, version INTEGER)",
        (),
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS kv_tenant_idx ON kv(tenant)", ())?;

    // Open the ack log with the documented durability contract: see
    // `open_ack_log` for fsync details.
    let mut ack = open_ack_log(&args.ack_log)?;

    // NOW arm the failpoint. For panic-style cases with
    // `kill_after_n_hits > 1` we route through a counted-callback
    // failpoint (`cfg_skip_then_panic`) so the workload actually
    // survives the first K-1 hits and crashes on the K-th. The
    // `K*panic` grammar in fail 0.5.x means "apply panic up to K
    // times" — i.e. panic on the FIRST hit — which is the wrong
    // semantic and is what produced the previous false-pass results
    // (the workload died during schema setup before any commit
    // acked, the gate had no oracle, and the case trivially
    // satisfied `lost <= 0`).
    //
    // Other actions (`return`, `off`, `sleep`, etc.) are forwarded
    // through the string `cfg` path, where the kernel validator
    // rejects unknown task tokens like `abort` loudly.
    if args.action.trim() == "panic" && args.kill_after_n_hits > 1 {
        let skip = (args.kill_after_n_hits - 1) as usize;
        eprintln!(
            "failpoint-child: arming failpoint={} action=skip-then-panic (skip={skip}, kill_after_n_hits={})",
            args.failpoint, args.kill_after_n_hits,
        );
        redlinedb_kernel::failpoints::cfg_skip_then_panic(&args.failpoint, skip)
            .map_err(|err| anyhow::anyhow!("arm failpoint {}: {}", args.failpoint, err))?;
    } else {
        let action = apply_kill_count(&args.action, args.kill_after_n_hits);
        eprintln!(
            "failpoint-child: arming failpoint={} action={} (raw={})",
            args.failpoint, action, args.action
        );
        redlinedb_kernel::failpoints::cfg(&args.failpoint, &action)
            .map_err(|err| anyhow::anyhow!("arm failpoint {}: {}", args.failpoint, err))?;
    }

    // Every 16 rows the child also calls `Database::checkpoint()` so
    // failpoints that gate the checkpoint path (`engine::checkpoint`,
    // `wal::flush_all`, `storage::control::write`) actually fire.
    // Without this hook the simple INSERT loop would never exercise
    // those sites and the corresponding matrix cases would silently
    // pass.
    const CHECKPOINT_EVERY_ROWS: usize = 16;

    for key in 0..args.rows {
        // Use INSERT OR REPLACE so the same key can be retried after
        // a successful commit if our process is restarted; under the
        // matrix runner each child has a unique db_dir so this never
        // triggers, but keeping the SQL idempotent matches the
        // recover.rs convention.
        let result = (|| -> Result<()> {
            let params = vec![
                redlinedb::Value::Integer(key as i64),
                redlinedb::Value::Integer((key % 32) as i64),
                redlinedb::Value::Blob(format!("value-{key:08}").into_bytes().into()),
                redlinedb::Value::Integer(1),
            ];
            conn.begin(redlinedb::BeginMode::Immediate)?;
            conn.execute(
                "INSERT OR REPLACE INTO kv(k, tenant, v, version) VALUES (?, ?, ?, ?)",
                params,
            )?;
            conn.commit()?;
            Ok(())
        })();
        if result.is_err() {
            // Failpoint that returned an error; the parent still gets
            // a meaningful ack count up to this point.
            return Ok(());
        }
        ack_row(&mut ack, key)?;

        if key > 0 && key.is_multiple_of(CHECKPOINT_EVERY_ROWS) && db.checkpoint().is_err() {
            // The checkpoint failpoint fires here; treat error as
            // workload termination so the parent observes a clean
            // ack-up-to-N before our process aborts.
            return Ok(());
        }
    }
    Ok(())
}

fn run_case(
    engine: EngineKind,
    durability: DurabilityKind,
    case: &FailpointMatrixCase,
    kill_after_n_hits: u64,
) -> Result<FailpointMatrixRun> {
    let tmp = tempfile::tempdir()
        .with_context(|| format!("create failpoint matrix tempdir for {}", case.name))?;
    let db_dir = tmp.path().join(format!("{engine:?}-{}", case.name));
    let ack_log = tmp.path().join(format!("{engine:?}-{}.ack", case.name));

    let exe = std::env::current_exe()?;
    let mut command = Command::new(exe);
    command
        .arg("failpoint-child")
        .arg("--engine")
        .arg(engine_name(engine))
        .arg("--durability")
        .arg(durability.as_str())
        .arg("--db-dir")
        .arg(&db_dir)
        .arg("--ack-log")
        .arg(&ack_log)
        .arg("--failpoint")
        .arg(&case.failpoint)
        .arg("--action")
        .arg(&case.action)
        .arg("--rows")
        .arg(case.rows.max(1).to_string())
        .arg("--kill-after-n-hits")
        .arg(kill_after_n_hits.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let status = command
        .status()
        .with_context(|| format!("spawn failpoint child for case {}", case.name))?;

    let acknowledged = read_ack_count(&ack_log)?;
    let recovered = match verify_recovered(durability, &db_dir) {
        Ok(value) => value,
        Err(err) => {
            eprintln!(
                "failpoint-matrix:   verify_recovered failed for {}: {err:#}",
                case.name
            );
            0
        }
    };
    let lost = acknowledged as i64 - recovered as i64;
    let lost_clamped = lost.max(0);

    let child_exit_status = status.code();
    let observed = ObservedRun {
        child_exit_status,
        child_exit_success: status.success(),
        acknowledged,
        recovered,
        lost_acked_commits: lost_clamped,
    };
    let (passed, pass_reason) = evaluate_verdict(case, &observed);

    Ok(FailpointMatrixRun {
        case: case.name.clone(),
        failpoint: case.failpoint.clone(),
        action: case.action.clone(),
        engine,
        durability,
        kill_after_n_hits,
        child_status: format_status(child_exit_status, status.success()),
        child_exit_status,
        acknowledged,
        recovered,
        lost_acked_commits: lost_clamped,
        passed,
        pass_reason,
    })
}

/// Snapshot of what the parent observed about a single child run.
/// The verdict-evaluator pulls everything it needs from this struct,
/// keeping it independent of `std::process::ExitStatus` so unit tests
/// can construct synthetic observations without spawning a child.
#[derive(Debug, Clone)]
pub struct ObservedRun {
    pub child_exit_status: Option<i32>,
    pub child_exit_success: bool,
    pub acknowledged: usize,
    pub recovered: usize,
    pub lost_acked_commits: i64,
}

fn matches_expected_exit(expect: ExpectExit, observed: &ObservedRun) -> bool {
    match expect {
        ExpectExit::Any => true,
        ExpectExit::Zero => observed.child_exit_success,
        // Non-zero covers both an exit code != 0 and signal-deaths
        // (e.g. SIGABRT from the panic hook), where `code()` returns
        // None. A successful exit (code 0) does NOT count as the
        // expected death.
        ExpectExit::NonZero => !observed.child_exit_success,
    }
}

/// Apply the lane-fp three-clause gate to a `(case, observed)` pair.
///
/// Pass condition is the AND of:
///
/// 1. `child_exit_success` matches `case.expect_child_exit`,
/// 2. `acknowledged > 0` OR `case.expect_zero_acks == true`,
/// 3. `lost_acked_commits == 0`.
///
/// Returns `(passed, reason)` where `reason` is a human-readable
/// summary suitable for the per-case JSON report. Public so the
/// bench-level tests can construct synthetic observations and
/// exercise the verdict logic without spawning a child binary.
pub fn evaluate_verdict(case: &FailpointMatrixCase, observed: &ObservedRun) -> (bool, String) {
    let exit_ok = matches_expected_exit(case.expect_child_exit, observed);
    let acks_ok = observed.acknowledged > 0 || case.expect_zero_acks;
    let no_lost = observed.lost_acked_commits == 0;
    let passed = exit_ok && acks_ok && no_lost;

    let mut failures: Vec<String> = Vec::new();
    if !exit_ok {
        let observed_str = format_status(observed.child_exit_status, observed.child_exit_success);
        failures.push(format!(
            "child exit {observed_str} did not match expect_child_exit={:?}",
            case.expect_child_exit
        ));
    }
    if !acks_ok {
        failures.push(
            "acknowledged=0 with expect_zero_acks=false (oracle would be vacuous; \
             set expect_zero_acks=true if the failpoint legitimately fires before \
             any commit acks)"
                .to_owned(),
        );
    }
    if !no_lost {
        failures.push(format!(
            "lost_acked_commits={} (acknowledged={} recovered={})",
            observed.lost_acked_commits, observed.acknowledged, observed.recovered,
        ));
    }
    let reason = if passed {
        format!(
            "exit matched {:?}, acknowledged={} (>=1 or expect_zero_acks), \
             lost_acked_commits=0",
            case.expect_child_exit, observed.acknowledged,
        )
    } else {
        format!("verdict=FAIL: {}", failures.join("; "))
    };
    (passed, reason)
}

fn verify_recovered(durability: DurabilityKind, db_dir: &Path) -> Result<usize> {
    let mut options = redlinedb::OpenOptions::default();
    options.memory.cache_bytes = 8 * 1024 * 1024;
    // CRITICAL: `create: true` (the default) routes the redlinedb facade
    // through `Database::create`, which re-initialises the page file
    // and wipes any existing data. For recovery verification we need
    // `Database::open`, which replays the WAL on top of the existing
    // page file.
    options.create = false;
    options.durability = match durability {
        DurabilityKind::Strict => redlinedb::Durability::Strict,
        DurabilityKind::Normal => redlinedb::Durability::Normal,
        DurabilityKind::Unsafe => redlinedb::Durability::UnsafeDev,
    };
    let path = db_dir.join("bench.redline");
    let db = redlinedb::Database::open_with_options(&path, options)
        .with_context(|| format!("open recovered db at {}", path.display()))?;
    let mut conn = db.connect().with_context(|| "connect to recovered db")?;
    // The kv table may not exist if the child died before the
    // CREATE TABLE durably committed. Treat that as zero recovered
    // rows rather than propagating the error: the gate is about
    // whether *acknowledged* rows are recovered, and ack count is
    // zero in that scenario, so the case naturally passes.
    let count = match conn.query("SELECT COUNT(*) FROM kv", ()) {
        Ok(mut rows) => match rows.step()? {
            redlinedb::Step::Row(row) => match row.get_ref(0)? {
                redlinedb::ValueRef::Integer(value) => value,
                redlinedb::ValueRef::Null => 0,
                _ => 0,
            },
            redlinedb::Step::Done => 0,
        },
        Err(err) => {
            eprintln!(
                "failpoint-matrix:   recovered db at {} has no kv table ({err})",
                path.display()
            );
            0
        }
    };
    Ok(count.max(0) as usize)
}

fn read_ack_count(path: &Path) -> Result<usize> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err.into()),
    };
    let reader = BufReader::new(file);
    let mut count = 0_usize;
    for line in reader.lines() {
        let line = line?;
        if !line.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

fn engine_name(engine: EngineKind) -> &'static str {
    match engine {
        EngineKind::Redline => "redline",
        EngineKind::Sqlite => "sqlite",
    }
}

/// Open the ack log used by the failpoint-matrix child to record
/// contractually-durable rows.
///
/// Durability semantics:
///
/// - The file is opened with `create=true, append=true`. Append mode
///   gives us atomic per-line semantics on POSIX so concurrent writers
///   would not interleave (the matrix runner is single-threaded but
///   this matches the recovery-matrix child's ack-log convention).
/// - We `sync_all()` the file handle once after opening so the inode
///   metadata (size = 0, length zero) is on disk before the first
///   write. This matters when the workload crashes before the first
///   commit: without the initial fsync the directory entry might not
///   exist on disk and `read_ack_count` would return zero even when
///   the on-disk WAL contains acked rows.
/// - We `sync_all()` the parent directory so the directory entry that
///   names the ack log is itself durable. macOS HFS+/APFS and Linux
///   ext4 both require this for the file to survive a crash that
///   happens between `creat()` and the first fsync.
///
/// The returned [`File`] handle MUST be passed to [`ack_row`] for each
/// subsequent commit. Reopening the file per-write would defeat the
/// fsync contract and cost an extra `open(2)` per row.
pub fn open_ack_log(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open ack log {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("initial sync of ack log {}", path.display()))?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Ok(dir) = File::open(parent)
    {
        // Best-effort directory fsync: not all filesystems support it
        // (e.g. tmpfs on some kernels returns EINVAL) so we ignore the
        // result. Real on-disk filesystems used by the matrix harness
        // do support it.
        let _ = dir.sync_all();
    }
    Ok(file)
}

/// Append `{key}\n` to the ack log and fsync the file.
///
/// This is the per-row gate-oracle write. Every entry that returns
/// `Ok(())` is contractually durable: a subsequent process crash MUST
/// leave the line on disk for `read_ack_count` to discover. If the
/// engine then fails to recover the row, that constitutes a
/// lost-acked-commit and the strict gate fails.
///
/// `flush()` would only drain Rust's `BufWriter`; we use `sync_all()`
/// to issue a real fsync(2) that flushes the OS page cache. The
/// failpoint matrix's correctness depends on this distinction.
pub fn ack_row(file: &mut File, key: usize) -> Result<()> {
    writeln!(file, "{key}").with_context(|| format!("write ack row {key}"))?;
    file.sync_all()
        .with_context(|| format!("sync ack row {key}"))?;
    Ok(())
}

fn apply_kill_count(raw: &str, kill_after_n_hits: u64) -> String {
    // The `fail` crate accepts `K*action` to fire `K` times before
    // disarming. We only prepend the count when > 1 so the common
    // single-shot case stays readable in logs.
    //
    // Action strings are NOT rewritten here. The `fail` crate honours
    // `panic`, `return(value)`, `off`, `print(msg)`, `pause`,
    // `sleep(N)`, and `yield` directly. In particular `return` makes
    // the failpoint short-circuit the function, which the kernel's
    // `wal::flush` hook accepts via its closure form (returns
    // `Ok(written_lsn)` without fsync). Translating `return` to
    // `panic` here would silently change a "skipped fsync" test into
    // a "panic mid-fsync" test, which is the wrong semantics.
    let action = raw.trim();
    if kill_after_n_hits <= 1 {
        action.to_string()
    } else {
        format!("{kill_after_n_hits}*{action}")
    }
}

fn format_status(code: Option<i32>, success: bool) -> String {
    match code {
        Some(code) => format!("exit({code})"),
        None if success => "success".to_string(),
        None => "signal".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_kill_count_passes_actions_verbatim() {
        // Every action keyword the `fail` crate accepts must round-trip
        // unchanged at single-hit count.
        assert_eq!(apply_kill_count("panic", 1), "panic");
        assert_eq!(apply_kill_count("abort", 1), "abort");
        assert_eq!(apply_kill_count("return", 1), "return");
        assert_eq!(apply_kill_count("off", 1), "off");
        assert_eq!(apply_kill_count("pause", 1), "pause");
        assert_eq!(apply_kill_count("yield", 1), "yield");
        assert_eq!(apply_kill_count("sleep(10)", 1), "sleep(10)");
        assert_eq!(apply_kill_count("print(hello)", 1), "print(hello)");
        assert_eq!(apply_kill_count("return(7)", 1), "return(7)");
    }

    #[test]
    fn apply_kill_count_prepends_multiplier_when_count_exceeds_one() {
        assert_eq!(apply_kill_count("panic", 5), "5*panic");
        assert_eq!(apply_kill_count("return", 25), "25*return");
        // 50%action probability strings are also forwarded verbatim
        // when count==1; they compose with the `K*` prefix when count>1
        // (the `fail` crate parses both layered).
        assert_eq!(apply_kill_count("50%panic", 1), "50%panic");
        assert_eq!(apply_kill_count("50%panic", 3), "3*50%panic");
    }

    #[test]
    fn apply_kill_count_trims_whitespace() {
        assert_eq!(apply_kill_count("  panic  ", 1), "panic");
        assert_eq!(apply_kill_count("\treturn\n", 2), "2*return");
    }
}
