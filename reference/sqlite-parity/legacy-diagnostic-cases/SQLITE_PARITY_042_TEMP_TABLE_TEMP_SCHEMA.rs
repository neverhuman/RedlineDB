// Auto-generated SQLite parity case.
// Source: SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA

pub fn case() -> crate::ParityCase {
    crate::ParityCase {
        id: 42,
        folder: r"SQLITE_PARITY_042_TEMP_TABLE_TEMP_SCHEMA",
        name: r"TEMP_TABLE_TEMP_SCHEMA",
        category: r"SQL_TEMP",
        priority: r"P0",
        profile: r"memory",
        kind: r"sql",
        description: r"CREATE TEMP TABLE and sqlite_temp_schema visibility.",
        status: r"active",
        db: r":memory:",
        args: &[],
        stdin: r".mode list
.headers off
.separator |
.nullvalue NULL
CREATE TEMP TABLE tt(x INT);
INSERT INTO tt VALUES(3);
SELECT name FROM sqlite_temp_schema WHERE type='table' AND name='tt';
SELECT x FROM tt;
",
        expected_exit: 0,
        compare_stdout: true,
        expected_stdout: Some(r"tt
3
"),
        expected_stdout_contains: &[],
        expected_stderr_contains: &[],
        expected_combined_contains: &[],
        files: &[],
        script: None,
        notes: r"",
    }
}
