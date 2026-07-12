// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 212,
        folder: r"SQLITE_PARITY_212_SQL_VACUUM_INTO_TEMPFILE",
        name: r"SQL_VACUUM_INTO_TEMPFILE",
        category: r"SQL_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"sql",
        description: r"VACUUM INTO short-lived temp database file.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(x INT);
INSERT INTO t VALUES(12);
VACUUM INTO '{{TMP}}/vac.db';
ATTACH '{{TMP}}/vac.db' AS v;
SELECT x FROM v.t;
DETACH v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"12
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
