// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_021_UPSERT_DO_NOTHING

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 21,
        folder: r"SQLITE_PARITY_021_UPSERT_DO_NOTHING",
        name: r"UPSERT_DO_NOTHING",
        category: r"SQL_UPSERT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT ... ON CONFLICT DO NOTHING.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT PRIMARY KEY, v INT);
INSERT INTO t VALUES('a',1);
INSERT INTO t(k,v) VALUES('a',2) ON CONFLICT(k) DO NOTHING;
SELECT count(*), max(v) FROM t;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
