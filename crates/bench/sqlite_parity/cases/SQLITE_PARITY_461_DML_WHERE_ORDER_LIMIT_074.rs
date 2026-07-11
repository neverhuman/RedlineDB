// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 461,
        folder: r"SQLITE_PARITY_461_DML_WHERE_ORDER_LIMIT_074",
        name: r"DML_WHERE_ORDER_LIMIT_074",
        category: r"GEN_SQL_DML",
        priority: r"P1",
        profile: r"memory",
        kind: r"sql-generated",
        description: r"Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_074.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INTEGER PRIMARY KEY, label TEXT, bucket INT);
INSERT INTO t VALUES (1, 'v1', 1), (2, 'v2', 2), (3, 'v3', 3), (4, 'v4', 4), (5, 'v5', 0), (6, 'v6', 1), (7, 'v0', 2), (8, 'v1', 3);
UPDATE t SET bucket = bucket + 2 WHERE id % 2 = 0;
DELETE FROM t WHERE id = 3;
SELECT id, label, bucket FROM t WHERE bucket >= 4 ORDER BY bucket, id LIMIT 5;
SELECT count(*), min(id), max(id), sum(bucket) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"2|v2|4
8|v1|5
4|v4|6
7|1|8|21
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"Generated from deterministic matrix; expected output produced by Python sqlite3 3.46.1 during artifact creation.",
    }
}
