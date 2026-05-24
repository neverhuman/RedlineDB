//! D7 differential fuzz parity gate.
//!
//! For each iteration: generate one SQL statement via `sqlsmith`, execute
//! it against a fresh `rusqlite::Connection::open_in_memory()` AND a fresh
//! `redlinedb::Database::create_in_memory` connection, then compare
//! outcomes via `normalize::compare_outcomes`. Any divergence panics with
//! both engines' outputs pretty-printed for triage.
//!
//! Knobs:
//!   * `REDLINEDB_FUZZ_ITERS` (default 1000) — iteration count.
//!   * `REDLINEDB_FUZZ_SEED` (default 7) — RNG seed for reproducibility.
//!   * `REDLINEDB_FUZZ_BASELINE_RATE` — optional local comparison ceiling for
//!     the observed divergence rate. If unset, the test requires zero
//!     divergences.

use std::sync::Arc;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use redlinedb_bench::fuzz::normalize::{
    Cell, Divergence, Outcome, classify, compare_outcomes, is_ordered,
};
use redlinedb_bench::fuzz::sqlsmith::{SCHEMA_SQL, SEED_SQL, generate_stmt};

fn iters_from_env() -> usize {
    std::env::var("REDLINEDB_FUZZ_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000)
}

fn seed_from_env() -> u64 {
    std::env::var("REDLINEDB_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7)
}

fn rusqlite_value_to_cell(value: rusqlite::types::Value) -> Cell {
    match value {
        rusqlite::types::Value::Null => Cell::Null,
        rusqlite::types::Value::Integer(v) => Cell::Integer(v),
        rusqlite::types::Value::Real(v) => Cell::Real(v),
        rusqlite::types::Value::Text(v) => Cell::Text(v),
        rusqlite::types::Value::Blob(v) => Cell::Blob(v),
    }
}

fn rldb_value_to_cell(value: redlinedb::ValueRef<'_>) -> Cell {
    match value {
        redlinedb::ValueRef::Null => Cell::Null,
        redlinedb::ValueRef::Integer(v) => Cell::Integer(v),
        redlinedb::ValueRef::Real(v) => Cell::Real(v),
        redlinedb::ValueRef::Text(v) => Cell::Text(v.to_owned()),
        redlinedb::ValueRef::Blob(v) => Cell::Blob(v.to_owned()),
    }
}

fn run_sqlite(conn: &rusqlite::Connection, sql: &str) -> Outcome {
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            return Outcome::Error {
                class: classify(&msg),
                raw: msg,
            };
        }
    };
    let ncols = stmt.column_count();
    if ncols == 0 {
        // DDL/DML — execute and report Done.
        return match conn.execute(sql, []) {
            Ok(_) => Outcome::Done,
            Err(e) => {
                let msg = e.to_string();
                Outcome::Error {
                    class: classify(&msg),
                    raw: msg,
                }
            }
        };
    }
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut q = match stmt.query([]) {
        Ok(q) => q,
        Err(e) => {
            let msg = e.to_string();
            return Outcome::Error {
                class: classify(&msg),
                raw: msg,
            };
        }
    };
    loop {
        match q.next() {
            Ok(Some(row)) => {
                let mut current = Vec::with_capacity(ncols);
                for i in 0..ncols {
                    let value: rusqlite::types::Value = match row.get(i) {
                        Ok(v) => v,
                        Err(e) => {
                            let msg = e.to_string();
                            return Outcome::Error {
                                class: classify(&msg),
                                raw: msg,
                            };
                        }
                    };
                    current.push(rusqlite_value_to_cell(value));
                }
                rows.push(current);
            }
            Ok(None) => break,
            Err(e) => {
                let msg = e.to_string();
                return Outcome::Error {
                    class: classify(&msg),
                    raw: msg,
                };
            }
        }
    }
    Outcome::Rows(rows)
}

fn run_redline(conn: &mut redlinedb::Connection, sql: &str) -> Outcome {
    let mut stmt = match conn.prepare_owned(sql) {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            return Outcome::Error {
                class: classify(&msg),
                raw: msg,
            };
        }
    };
    let ncols = stmt.column_count();
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    loop {
        match stmt.step() {
            Ok(redlinedb::OwnedStep::Row) => {
                let mut current = Vec::with_capacity(ncols);
                for i in 0..ncols {
                    let value = match stmt.column_ref(i) {
                        Ok(v) => v,
                        Err(e) => {
                            let msg = e.to_string();
                            return Outcome::Error {
                                class: classify(&msg),
                                raw: msg,
                            };
                        }
                    };
                    current.push(rldb_value_to_cell(value));
                }
                rows.push(current);
            }
            Ok(redlinedb::OwnedStep::Done) => break,
            Err(e) => {
                let msg = e.to_string();
                return Outcome::Error {
                    class: classify(&msg),
                    raw: msg,
                };
            }
        }
    }
    if ncols == 0 {
        // RedlineDB reports prepared DML as no-column-step-Done.
        Outcome::Done
    } else {
        Outcome::Rows(rows)
    }
}

struct EnginePair {
    sqlite: rusqlite::Connection,
    redline_db: Arc<redlinedb::Database>,
    redline: redlinedb::Connection,
}

impl EnginePair {
    fn new() -> Self {
        let sqlite = rusqlite::Connection::open_in_memory().expect("rusqlite in-memory");
        let redline_db = Arc::new(
            redlinedb::Database::create_in_memory(redlinedb::OpenOptions::default())
                .expect("redlinedb in-memory"),
        );
        let mut redline = redline_db.connect().expect("redline connect");

        for sql in SCHEMA_SQL.iter().chain(SEED_SQL.iter()) {
            sqlite
                .execute_batch(sql)
                .unwrap_or_else(|err| panic!("rusqlite setup failed: {sql}\n{err}"));
            redline
                .execute(sql, ())
                .unwrap_or_else(|err| panic!("redline setup failed: {sql}\n{err:?}"));
        }

        Self {
            sqlite,
            redline_db,
            redline,
        }
    }
}

fn pretty_outcome(out: &Outcome) -> String {
    match out {
        Outcome::Done => "DONE".to_string(),
        Outcome::Rows(rows) => {
            let mut s = format!("{} rows:\n", rows.len());
            for (i, row) in rows.iter().take(10).enumerate() {
                s.push_str(&format!("  [{i}] "));
                for cell in row {
                    s.push_str(&format!("{cell:?} "));
                }
                s.push('\n');
            }
            if rows.len() > 10 {
                s.push_str(&format!("  ... and {} more\n", rows.len() - 10));
            }
            s
        }
        Outcome::Error { class, raw } => format!("ERR class={class:?} raw={raw:?}"),
    }
}

fn render_divergence(seed: u64, i: usize, sql: &str, div: &Divergence) -> String {
    format!(
        "\n========== FUZZ DIVERGENCE (seed={seed} iter={i}) ==========\n\
         reason: {reason}\n\
         sql:    {sql}\n\
         --- rusqlite ---\n{sqlite_pretty}\n\
         --- redlinedb ---\n{redline_pretty}\n\
         ============================================================\n",
        reason = div.reason,
        sqlite_pretty = pretty_outcome(&div.sqlite),
        redline_pretty = pretty_outcome(&div.redline),
    )
}

fn iter_known_skips(sql: &str) -> bool {
    // Skip only active tracked gaps. Implemented SQL surfaces must stay in
    // the fuzzer so parity regressions are caught before they reach users.
    let lower = sql.to_ascii_lowercase();
    // WS-A7: correlated subqueries — qualified outer-column refs in nested
    // SELECTs fail planning today (see crates/sql/tests/differential_lab.rs:171).
    // The scalar-subquery generator emits `WHERE t2.t1_id = t1.id` which
    // matches this gap exactly. Same for IN-subquery with correlation.
    if lower.contains("(select ") && lower.contains(" where ") {
        return true;
    }
    false
}

/// Read an optional local divergence-rate ceiling. This is an assertion input,
/// not a generated receipt; the test never writes parity evidence artifacts.
fn read_baseline_rate() -> Option<f64> {
    std::env::var("REDLINEDB_FUZZ_BASELINE_RATE")
        .ok()
        .and_then(|value| value.parse().ok())
}

#[test]
fn fuzz_parity_against_rusqlite() {
    let iters = iters_from_env();
    let seed = seed_from_env();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut pair = EnginePair::new();
    let mut skipped = 0_usize;
    let mut successes = 0_usize;
    let mut divergences: Vec<String> = Vec::new();
    let start = std::time::Instant::now();

    for i in 0..iters {
        let sql = generate_stmt(&mut rng);
        if iter_known_skips(&sql) {
            skipped += 1;
            continue;
        }

        let sqlite_outcome = run_sqlite(&pair.sqlite, &sql);
        let redline_outcome = run_redline(&mut pair.redline, &sql);
        let ordered = is_ordered(&sql);

        if let Err(div) = compare_outcomes(sqlite_outcome, redline_outcome, ordered) {
            let rendered = render_divergence(seed, i, &sql, &div);
            divergences.push(rendered);
        } else {
            successes += 1;
        }
    }
    let elapsed = start.elapsed();
    let observed = divergences.len();
    let denom = (successes + observed).max(1);
    let observed_rate = observed as f64 / denom as f64;
    let prior_baseline_rate = read_baseline_rate();

    eprintln!(
        "fuzz parity: iters={iters} successes={successes} skipped={skipped} \
         divergences={observed} rate={observed_rate:.4} elapsed={el:?}",
        el = elapsed,
    );

    // Keep a Database handle alive until here so the in-memory backing
    // store is dropped after the test completes (not mid-iteration).
    drop(pair.redline);
    drop(pair.sqlite);
    drop(pair.redline_db);

    // Sanity: at least 10% of iterations must have actually exercised both
    // engines (not been short-circuited by known skips). Without this floor
    // the test could trivially pass by skipping everything.
    assert!(
        successes + observed >= iters / 10,
        "fuzz parity exercised only {} of {iters} iterations; \
         too many known-skip statement kinds — gate is no longer meaningful",
        successes + observed,
    );

    // Optional local gate: divergence RATE (per executed iter) must not exceed
    // the configured baseline rate (with a small +10% safety margin to absorb
    // fuzzer non-determinism across dep upgrades). The
    // rate framing makes the gate iteration-count-independent: bumping
    // REDLINEDB_FUZZ_ITERS from 1000 to 100000 (nightly lane) does not
    // false-fail the gate. Without a configured baseline this only passes when
    // it observes zero divergences, so first-run drift cannot bless itself.
    let gate_failed = match prior_baseline_rate {
        Some(baseline) => observed_rate > baseline * 1.10 + 0.01,
        None => observed != 0,
    };

    assert!(
        !gate_failed,
        "fuzz parity divergence regressed: observed_rate={observed_rate:.4} \
         baseline_rate={prior_baseline_rate:?}\n\
         first 3 divergences:\n{first3}",
        first3 = divergences
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
