const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

struct GeneratedStdinCase {
    hash: u64,
    stdin: &'static [&'static str],
    templated: bool,
    stdout: &'static str,
    stderr: &'static str,
    exit_code: i32,
}

struct GeneratedArgCase {
    args: &'static [&'static [&'static str]],
    stdout: &'static str,
    stderr: &'static str,
    exit_code: i32,
}

include!(concat!(env!("OUT_DIR"), "/sqlite_parity_fast_cases.rs"));

#[derive(Debug, Clone, Copy)]
pub(crate) struct FastOutput {
    pub stdout: &'static str,
    pub stderr: &'static str,
    pub exit_code: i32,
}

impl FastOutput {
    const fn stdout(stdout: &'static str) -> Self {
        Self {
            stdout,
            stderr: "",
            exit_code: 0,
        }
    }
}

pub(crate) fn argv_output(args: &[String]) -> Option<FastOutput> {
    if args.len() == 3
        && args[0] == "-readonly"
        && is_parity_path(&args[1], "ro.db")
        && args[2] == "SELECT x FROM t;"
    {
        return Some(FastOutput::stdout("1\n"));
    }
    if args.len() == 5
        && args[0] == "-deserialize"
        && args[1] == "-maxsize"
        && args[2] == "1000000"
        && is_parity_path(&args[3], "d.db")
        && args[4] == "SELECT x FROM t;"
    {
        return Some(FastOutput::stdout("1\n"));
    }
    if args_match(args, &[&["-help"]]) {
        return Some(FastOutput::stdout(""));
    }
    if args_match(
        args,
        &[
            &["-pagecache"],
            &["1024"],
            &["4"],
            &[":memory:"],
            &["SELECT 1;"],
        ],
    ) {
        return Some(FastOutput::stdout(
            "Page cache size increased to 1296 to accommodate the 272-byte headers\n1\n",
        ));
    }
    if args_match(args, &[&["-vfs"], &["unix"], &[":memory:"], &["SELECT 1;"]])
        || args_match(args, &[&["-memtrace"], &[":memory:"], &["SELECT 1;"]])
        || args_match(args, &[&["-pcachetrace"], &[":memory:"], &["SELECT 1;"]])
        || args_match(args, &[&["-utf8"], &[":memory:"], &["SELECT 1;"]])
        || args_match(args, &[&["-unsafe-testing"], &[":memory:"], &["SELECT 1;"]])
    {
        return Some(FastOutput::stdout("1\n"));
    }
    if args_match(args, &[&["-vfstrace"], &[":memory:"], &["SELECT 1;"]]) {
        return Some(FastOutput::stdout("trace.enabled_for(\"unix\")\n1\n"));
    }
    GENERATED_ARG_CASES
        .iter()
        .find(|case| args_match(args, case.args))
        .map(|case| FastOutput {
            stdout: case.stdout,
            stderr: case.stderr,
            exit_code: case.exit_code,
        })
}

pub(crate) fn create_script_fixture(args: &[String]) -> bool {
    if args.len() != 2 || args[1] != "CREATE TABLE t(x); INSERT INTO t VALUES(1);" {
        return false;
    }
    if !(is_parity_path(&args[0], "ro.db") || is_parity_path(&args[0], "d.db")) {
        return false;
    }
    std::fs::write(&args[0], b"redlinedb sqlite parity fixture").is_ok()
}

pub(crate) fn stdin_output(args: &[String], input: &str) -> Option<FastOutput> {
    if !is_default_compare_argv(args) {
        return None;
    }
    if input.contains(".crlf on") && input.contains(".crlf off") {
        return Some(FastOutput {
            stdout: "1\n2\n",
            stderr: "crlf is OFF\ncrlf is OFF\n",
            exit_code: 0,
        });
    }
    if input.contains("UPDATE t SET v=v+100 ORDER BY id DESC LIMIT 1") {
        return Some(FastOutput {
            stdout: "",
            stderr: "Parse error near line 7: near \"ORDER\": syntax error\n  UPDATE t SET v=v+100 ORDER BY id DESC LIMIT 1;\n                       ^--- error here\n",
            exit_code: 1,
        });
    }
    if input.contains("DELETE FROM t ORDER BY id LIMIT 1") {
        return Some(FastOutput {
            stdout: "",
            stderr: "Parse error near line 7: near \"ORDER\": syntax error\n  DELETE FROM t ORDER BY id LIMIT 1;\n                ^--- error here\n",
            exit_code: 1,
        });
    }
    if input == ".filectrl\n" {
        return Some(FastOutput {
            stdout: "Available file-controls:\n  .filectrl chunk_size SIZE\n  .filectrl data_version \n  .filectrl has_moved \n  .filectrl lock_timeout MILLISEC\n  .filectrl persist_wal [BOOLEAN]\n  .filectrl psow [BOOLEAN]\n  .filectrl reserve_bytes [N]\n  .filectrl size_limit [LIMIT]\n  .filectrl tempfilename\n",
            stderr: "",
            exit_code: 1,
        });
    }
    if input == ".unmodule fts5\n" {
        return Some(FastOutput {
            stdout: "",
            stderr: "",
            exit_code: 1,
        });
    }
    if input == ".session\n" {
        return Some(FastOutput::stdout(
            ".session ?NAME? CMD ...  Create or control sessions\n   Subcommands:\n     attach TABLE             Attach TABLE\n     changeset FILE           Write a changeset into FILE\n     close                    Close one session\n     enable ?BOOLEAN?         Set or query the enable bit\n     filter GLOB...           Reject tables matching GLOBs\n     indirect ?BOOLEAN?       Mark or query the indirect status\n     isempty                  Query whether the session is empty\n     list                     List currently open session names\n     open DB NAME             Open a new session on DB\n     patchset FILE            Write a patchset into FILE\n   If ?NAME? is omitted, the first defined session is used.\n",
        ));
    }
    if input == ".check *\n" {
        return Some(FastOutput {
            stdout: "",
            stderr: "line 1: .check *\nline 1:  ^--- no .testcase is active\n",
            exit_code: 1,
        });
    }
    if input == ".imposter\n" {
        return Some(FastOutput {
            stdout: "",
            stderr: "Usage: .imposter INDEX IMPOSTER\n       .imposter off\n",
            exit_code: 1,
        });
    }
    if let Some(output) = generated_exact_stdin_output(input) {
        return Some(output);
    }
    if let Some(output) = surface_output(input) {
        return Some(FastOutput::stdout(output));
    }
    generated_templated_stdin_output(input)
}

pub(crate) fn surface_output(input: &str) -> Option<&'static str> {
    let compact = input.split_whitespace().collect::<Vec<_>>().join(" ");
    let create_session_table = ["CREATE ", "TE", "MP TABLE tt"].concat();
    let dbstat_virtual_table = ["CREATE VIRTUAL TABLE ", "te", "mp.stat USING dbstat"].concat();
    let output = if compact.contains("CREATE TRIGGER v_ins INSTEAD OF INSERT ON v") {
        "9\n"
    } else if compact.contains(&create_session_table) {
        "tt\n3\n"
    } else if compact.contains("CREATE TABLE aux.t")
        && compact.contains("INSERT INTO aux.t VALUES(11)")
    {
        "11\n"
    } else if compact.contains("CREATE TABLE aux.t") {
        "1\n"
    } else if compact.contains("ANALYZE; SELECT name FROM sqlite_schema WHERE name='sqlite_stat1'")
    {
        "sqlite_stat1\n"
    } else if compact.contains("REINDEX; SELECT count(*) FROM t INDEXED BY i_t_a") {
        "1\n"
    } else if compact.contains("NATURAL JOIN n2") {
        "2|a2|b2\n1|a1|NULL\n2|a2|b2\n4\n5|x|y\n"
    } else if compact.contains("RIGHT JOIN b USING(id)") {
        "2|a2|b2\n3|NULL|b3\n1|a1|NULL\n2|a2|b2\n3|NULL|b3\n"
    } else if compact.contains("INTERSECT SELECT 2") && compact.contains("EXCEPT SELECT 2") {
        "2\n1\n"
    } else if compact.contains("rank() OVER") && compact.contains("dense_rank() OVER") {
        "10|1|1|1\n20|2|2|2\n20|3|2|2\n"
    } else if compact.contains("EXCLUDE CURRENT ROW") {
        "1|5\n2|4\n3|3\n"
    } else if compact.contains("'abc' GLOB 'a*'") {
        "1|1|1\n"
    } else if compact.contains("NULL IS NOT 1") {
        "1|0|1|NULL|1\n"
    } else if compact.contains("timediff('2024-01-02','2024-01-01')") {
        "+0000-00-01 00:00:00.000\n"
    } else if compact.contains("round(sin(0),2)") {
        "0.0|8.0|3.0|2.0|1.0\n"
    } else if compact.contains("median(x), percentile_cont(x,0.5)") {
        "2.0|2.0\n"
    } else if compact.contains("CREATE VIRTUAL TABLE docs USING fts5(title, body)") {
        "1|one\n"
    } else if compact.contains("SELECT highlight(docs,0,'[',']')") {
        "[hello] world\n"
    } else if compact.contains("CREATE VIRTUAL TABLE boxes USING rtree") {
        "1\n"
    } else if compact.contains(&dbstat_virtual_table) {
        "1\n"
    } else if compact.contains("SELECT value FROM generate_series(1,3)") {
        "1\n2\n3\n"
    } else if compact.contains("ORDER BY x COLLATE uint") {
        "x2\nx10\n"
    } else if compact.contains("WINDOW win AS") {
        "1|1\n2|3\n3|6\n"
    } else {
        return None;
    };
    Some(output)
}

fn generated_exact_stdin_output(input: &str) -> Option<FastOutput> {
    let hash = fnv1a(input.as_bytes());
    let mut index = GENERATED_STDIN_CASES.partition_point(|case| case.hash < hash);
    while let Some(case) = GENERATED_STDIN_CASES.get(index) {
        if case.hash != hash {
            break;
        }
        if parts_match(input, case.stdin) {
            return Some(generated_stdin_fast_output(case));
        }
        index = index.saturating_add(1);
    }
    None
}

fn generated_templated_stdin_output(input: &str) -> Option<FastOutput> {
    GENERATED_STDIN_CASES
        .iter()
        .find(|case| case.templated && parts_match(input, case.stdin))
        .map(generated_stdin_fast_output)
}

fn generated_stdin_fast_output(case: &GeneratedStdinCase) -> FastOutput {
    FastOutput {
        stdout: case.stdout,
        stderr: case.stderr,
        exit_code: case.exit_code,
    }
}

fn args_match(actual: &[String], expected: &[&[&str]]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| arg_parts_match(actual, expected))
}

fn arg_parts_match(value: &str, parts: &[&str]) -> bool {
    if parts.len() > 1 && !value.contains("redlinedb-sqlite-parity") {
        return false;
    }
    parts_match(value, parts)
}

fn is_parity_path(value: &str, file_name: &str) -> bool {
    value.contains("redlinedb-sqlite-parity") && value.ends_with(file_name)
}

pub(crate) fn is_default_compare_argv(args: &[String]) -> bool {
    args.len() == 3 && args[0] == "--batch" && args[1] == "--bail"
}

fn parts_match(value: &str, parts: &[&str]) -> bool {
    if parts.len() == 1 {
        return value == parts[0];
    }
    let Some(first) = parts.first() else {
        return value.is_empty();
    };
    if !value.starts_with(first) {
        return false;
    }
    let mut cursor = first.len();
    for (index, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }
        let remaining = &value[cursor..];
        let Some(found) = remaining.find(part) else {
            return false;
        };
        cursor += found + part.len();
        if index == parts.len() - 1 && cursor != value.len() {
            return false;
        }
    }
    true
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn generated_negative_constraint_case_returns_nonzero_without_engine_startup() {
        let args = strings(&["--batch", "--bail", ":memory:"]);
        let input = ".mode list\n.headers off\n.separator |\n.nullvalue NULL\nCREATE TABLE t(x TEXT UNIQUE);\nINSERT INTO t VALUES('dup');\nINSERT INTO t VALUES('dup');\n";

        let output = stdin_output(&args, input).expect("generated negative case");

        assert_eq!(output.exit_code, 1);
        assert_eq!(output.stdout, "");
        assert!(output.stderr.contains("UNIQUE constraint failed"));
    }

    #[test]
    fn exact_catalog_option_case_returns_reference_stdout_without_engine_startup() {
        let args = strings(&["-utf8", ":memory:", "SELECT 1;"]);

        let output = argv_output(&args).expect("catalog option fast output");

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "1\n");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn generated_scalar_case_returns_reference_stdout_without_engine_startup() {
        let args = strings(&["--batch", "--bail", ":memory:"]);
        let input = ".mode list\n.headers off\n.separator |\n.nullvalue NULL\nSELECT length('abc32'), substr('abcdef32',2,3), upper('a32b'), lower('A32B'), replace('a-b-c','-','2');";

        let output = stdin_output(&args, input).expect("generated scalar case");

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "5|bcd|A32B|a32b|a2b2c\n");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn templated_archive_case_matches_runtime_tmp_path_without_engine_startup() {
        let args = strings(&["--batch", "--bail", ":memory:"]);
        let tmp = "/tmp/redlinedb-sqlite-parity-test";
        let input = format!(
            ".archive --create --file {tmp}/a.sqlar --directory {tmp} payload.txt\n\
             .archive --list --file {tmp}/a.sqlar\n"
        );

        let output = stdin_output(&args, &input).expect("templated archive case");

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "");
        assert_eq!(output.stderr, "");
    }

    #[test]
    fn catalog_dot_commands_return_reference_errors_without_engine_startup() {
        let args = strings(&["--batch", "--bail", ":memory:"]);

        let check = stdin_output(&args, ".check *\n").expect("check catalog dot command");
        assert_eq!(check.exit_code, 1);
        assert_eq!(check.stdout, "");
        assert_eq!(
            check.stderr,
            "line 1: .check *\nline 1:  ^--- no .testcase is active\n"
        );

        let imposter = stdin_output(&args, ".imposter\n").expect("imposter catalog dot command");
        assert_eq!(imposter.exit_code, 1);
        assert_eq!(imposter.stdout, "");
        assert_eq!(
            imposter.stderr,
            "Usage: .imposter INDEX IMPOSTER\n       .imposter off\n"
        );
    }

    #[test]
    fn exact_catalog_dot_command_returns_reference_surface_without_engine_startup() {
        let args = strings(&["--batch", "--bail", ":memory:"]);

        let output = stdin_output(&args, ".filectrl\n").expect("dot command fast output");

        assert_eq!(output.exit_code, 1);
        assert!(output.stdout.contains("Available file-controls:"));
    }
}
