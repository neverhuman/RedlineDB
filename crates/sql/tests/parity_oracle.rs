//! Differential SQL parity oracle (Workstream A1).
//!
//! Drives the seed corpus under `tests/parity_corpus/<tag>/*.sql` through
//! both `rusqlite` and `redlinedb_sql`, normalises the result, and writes a
//! baseline divergence summary under
//! `target/proof/sqlite-full-parity/baseline-divergence.txt`.
//!
//! The harness itself lives at `tests/parity_oracle/harness.rs` so it can
//! grow without bloating this test wrapper. The corpus directory layout
//! drives the `#[test]` matrix below; each tag (basic, compound, …) gets
//! one test function so failures localise to a feature surface.
//!
//! NOTE: this test is intentionally *non-fatal* on per-file mismatches
//! (it logs them, then writes a divergence summary). The test only fails
//! if the harness itself cannot run (e.g. no corpus files for a tag).
//! That is by design: A1 is the *foundation*; subsequent workstreams
//! will tighten the gate to fail on any divergence.

#[path = "parity_oracle/harness.rs"]
mod harness;

use std::fs;
use std::path::{Path, PathBuf};

const TAGS: &[&str] = &[
    "basic", "compound", "cte", "window", "view", "trigger", "index", "fk", "pragma", "json",
    "datetime",
];

fn corpus_dir(tag: &str) -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    Path::new(&manifest)
        .join("tests")
        .join("parity_corpus")
        .join(tag)
}

fn proof_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let workspace_root = Path::new(&manifest)
        .ancestors()
        .nth(2)
        .expect("workspace root");
    workspace_root
        .join("target")
        .join("proof")
        .join("sqlite-full-parity")
}

fn list_sql_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("sql") {
            out.push(p);
        }
    }
    out.sort();
    out
}

#[derive(Debug, Default)]
struct TagReport {
    tag: String,
    files: usize,
    passed: usize,
    failures: Vec<(String, String)>, // (file name, diff)
}

fn run_tag(tag: &str) -> TagReport {
    let mut report = TagReport {
        tag: tag.to_owned(),
        ..Default::default()
    };
    let dir = corpus_dir(tag);
    for path in list_sql_files(&dir) {
        report.files += 1;
        let sql = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_owned();
                report.failures.push((name, format!("read error: {e}")));
                continue;
            }
        };
        match harness::check_parity(&sql) {
            Ok(()) => report.passed += 1,
            Err(diff) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_owned();
                report.failures.push((name, diff));
            }
        }
    }
    report
}

fn write_summary(reports: &[TagReport]) {
    let dir = proof_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("could not create proof dir {}: {e}", dir.display());
        return;
    }
    let mut out = String::new();
    out.push_str("# SQLite parity oracle — baseline divergence\n");
    out.push_str("# Tag | Files | Passed | Failed\n");
    let mut total_files = 0usize;
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    for r in reports {
        let failed = r.failures.len();
        total_files += r.files;
        total_passed += r.passed;
        total_failed += failed;
        out.push_str(&format!(
            "{:<10} {:>5} {:>6} {:>6}\n",
            r.tag, r.files, r.passed, failed
        ));
    }
    out.push_str(&format!(
        "TOTAL      {:>5} {:>6} {:>6}\n\n",
        total_files, total_passed, total_failed
    ));
    for r in reports {
        if r.failures.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "\n## Failures in {} ({}/{})\n",
            r.tag,
            r.failures.len(),
            r.files
        ));
        for (name, diff) in &r.failures {
            out.push_str(&format!("\n### {}\n", name));
            out.push_str(diff);
            out.push('\n');
        }
    }
    let path = dir.join("baseline-divergence.txt");
    if let Err(e) = fs::write(&path, out) {
        eprintln!("could not write {}: {e}", path.display());
    }
}

#[test]
fn parity_oracle_baseline() {
    let reports: Vec<TagReport> = TAGS.iter().map(|t| run_tag(t)).collect();
    write_summary(&reports);

    // Print a one-line summary to stdout so CI logs surface the number.
    let total: (usize, usize, usize) = reports.iter().fold((0, 0, 0), |(f, p, e), r| {
        (f + r.files, p + r.passed, e + r.failures.len())
    });
    println!(
        "parity_oracle: {} files; {} pass; {} divergent",
        total.0, total.1, total.2
    );

    // Ensure each tag has at least 5 files so the foundation is real.
    for r in &reports {
        assert!(
            r.files >= 5,
            "parity tag {} has only {} files; the A1 corpus floor is 5 per tag",
            r.tag,
            r.files
        );
    }
}

#[test]
fn harness_self_test_basic_select() {
    // Smoke: a trivial constant SELECT should pass parity on both engines.
    harness::assert_parity("SELECT 1");
    harness::assert_parity("SELECT 1, 2, 'hello'");
}

#[test]
fn harness_self_test_setup_then_query() {
    harness::assert_parity(
        "CREATE TABLE t(a INTEGER, b TEXT);\n\
         INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c');\n\
         SELECT a, b FROM t ORDER BY a",
    );
}
