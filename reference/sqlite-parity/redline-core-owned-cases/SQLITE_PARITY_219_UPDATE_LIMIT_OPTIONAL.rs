// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 219,
        folder: r"SQLITE_PARITY_219_UPDATE_LIMIT_OPTIONAL",
        name: r"UPDATE_LIMIT_OPTIONAL",
        category: r"SQL_UPDATE_OPTIONAL",
        priority: r"P3",
        profile: r"memory",
        kind: r"sql",
        description: r"UPDATE ... ORDER BY ... LIMIT when compiled with update/delete limit support.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TABLE t(id INT PRIMARY KEY, v INT);
INSERT INTO t VALUES(1,10),(2,20),(3,30);
UPDATE t SET v=v+100 ORDER BY id DESC LIMIT 1;
SELECT id,v FROM t ORDER BY id;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1|10
2|20
3|130
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
