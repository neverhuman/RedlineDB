// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_020_UPSERT_DO_UPDATE

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 20,
        folder: r"SQLITE_PARITY_020_UPSERT_DO_UPDATE",
        name: r"UPSERT_DO_UPDATE",
        category: r"SQL_UPSERT",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"INSERT ... ON CONFLICT DO UPDATE.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT PRIMARY KEY, v INT);
INSERT INTO t VALUES('a',1);
INSERT INTO t(k,v) VALUES('a',2)
  ON CONFLICT(k) DO UPDATE SET v=excluded.v+10
  RETURNING k,v;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|12
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
