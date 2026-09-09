// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 213,
        folder: r"SQLITE_PARITY_213_SQL_WAL_CHECKPOINT_TEMPFILE",
        name: r"SQL_WAL_CHECKPOINT_TEMPFILE",
        category: r"SQL_TEMPFILE",
        priority: r"P2",
        profile: r"tempfile",
        kind: r"sql",
        description: r"PRAGMA journal_mode=WAL and wal_checkpoint on temp database file.",
        status: r"active",
        db: r"{{TMP}}/wal.db",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
PRAGMA journal_mode=WAL;
CREATE TABLE t(x);
INSERT INTO t VALUES(1);
PRAGMA wal_checkpoint(PASSIVE);
",
        expected_exit: 0,
        compare_stdout: false,
        expected_stdout: None,
        expected_stdout_contains: &[r"wal"],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
