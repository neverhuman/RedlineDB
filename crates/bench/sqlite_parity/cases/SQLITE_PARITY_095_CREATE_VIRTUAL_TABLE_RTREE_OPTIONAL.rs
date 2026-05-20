// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 95,
        folder: r"SQLITE_PARITY_095_CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL",
        name: r"CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL",
        category: r"SQL_VIRTUAL_TABLE_OPTIONAL",
        priority: r"P2",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE VIRTUAL TABLE USING rtree.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE VIRTUAL TABLE boxes USING rtree(id, x1, x2, y1, y2);
INSERT INTO boxes VALUES(1,0,10,0,10),(2,20,30,20,30);
SELECT id FROM boxes WHERE x1>=0 AND x2<=10;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"1
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
