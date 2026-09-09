// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_058_GROUP_BY_HAVING

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 58,
        folder: r"SQLITE_PARITY_058_GROUP_BY_HAVING",
        name: r"GROUP_BY_HAVING",
        category: r"SQL_AGGREGATE",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"GROUP BY and HAVING.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(k TEXT, v INT);
INSERT INTO t VALUES('a',1),('a',2),('b',1);
SELECT k,sum(v) FROM t GROUP BY k HAVING sum(v)>2 ORDER BY k;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"a|3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
