// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 44,
        folder: r"SQLITE_PARITY_044_ANALYZE_SQLITE_STAT1",
        name: r"ANALYZE_SQLITE_STAT1",
        category: r"SQL_ANALYZE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"ANALYZE creates sqlite_stat1 for indexed data.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(a INT);
CREATE INDEX i_t_a ON t(a);
INSERT INTO t VALUES(1),(2),(3);
ANALYZE;
SELECT name FROM sqlite_schema WHERE name='sqlite_stat1';
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"sqlite_stat1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
