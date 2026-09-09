// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 78,
        folder: r"SQLITE_PARITY_078_ON_CONFLICT_ALGORITHMS",
        name: r"ON_CONFLICT_ALGORITHMS",
        category: r"SQL_CONFLICT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT OR IGNORE and INSERT OR REPLACE conflict algorithms.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT PRIMARY KEY, v INT);
INSERT INTO t VALUES('a',1);
INSERT OR IGNORE INTO t VALUES('a',9);
INSERT OR REPLACE INTO t VALUES('a',2);
SELECT k,v,count(*) OVER () FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|2|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
