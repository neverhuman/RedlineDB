//! Matrix-product shard generators. Each generator returns a Vec<Case> with
//! expected outputs captured live from sqlite3, so by construction every
//! generated case ships ship-gate-green.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::sqlite_runner::{Case, PRELUDE, execute};

/// One generator. The recipe owns its own axes; `produce` returns the shard's
/// shard cases in canonical order.
struct Generator {
    name: &'static str,
    shard_filename: &'static str,
    category: &'static str,
    priority: &'static str,
    id_start: u64,
    required_capabilities: &'static [&'static str],
    produce: fn(&GenConfig) -> Result<Vec<RawCase>>,
}

pub struct GenConfig<'a> {
    pub category: &'a str,
    pub priority: &'a str,
    pub id_start: u64,
    pub required_capabilities: &'a [&'static str],
}

/// A pre-execution case description; `materialize` runs it through sqlite3
/// to capture stdout/exit and turn it into a final Case.
pub struct RawCase {
    pub name: String,
    pub description: String,
    pub stdin: String,
    pub prelude: bool,
    pub args: Vec<String>,
    pub compare_stdout: bool,
    pub expected_stdout_contains: Vec<String>,
    pub expected_stderr_contains: Vec<String>,
    pub expect_failure: bool,
}

impl RawCase {
    fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        stdin: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            stdin: stdin.into(),
            prelude: true,
            args: Vec::new(),
            compare_stdout: true,
            expected_stdout_contains: Vec::new(),
            expected_stderr_contains: Vec::new(),
            expect_failure: false,
        }
    }
}

fn materialize(raw: RawCase, id: u64, cfg: &GenConfig, sqlite_bin: &str) -> Result<Case> {
    let assembled = if raw.prelude {
        format!("{PRELUDE}{}", raw.stdin)
    } else {
        raw.stdin.clone()
    };
    let folder = format!("SQLITE_PARITY_{:05}_{}", id, raw.name);
    let provisional = Case {
        id,
        folder: folder.clone(),
        name: raw.name.clone(),
        category: cfg.category.to_owned(),
        priority: cfg.priority.to_owned(),
        profile: "memory".to_owned(),
        kind: "sql".to_owned(),
        description: raw.description.clone(),
        status: "active".to_owned(),
        db: ":memory:".to_owned(),
        args: raw.args.clone(),
        stdin: assembled.clone(),
        expected_exit: 0,
        compare_stdout: raw.compare_stdout,
        expected_stdout: None,
        expected_stdout_contains: raw.expected_stdout_contains.clone(),
        expected_stderr_contains: raw.expected_stderr_contains.clone(),
        expected_combined_contains: Vec::new(),
        files: Vec::new(),
        script: None,
        notes: String::new(),
        required_capabilities: cfg
            .required_capabilities
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
    };
    let out = execute(&provisional, sqlite_bin)?;
    if raw.expect_failure && out.exit_code == 0 {
        bail!(
            "generator {} expected failure but sqlite3 returned 0",
            raw.name
        );
    }
    if !raw.expect_failure && out.exit_code != 0 {
        bail!(
            "generator {} unexpected sqlite3 failure exit={} stderr={:?}",
            raw.name,
            out.exit_code,
            out.stderr
        );
    }
    Ok(Case {
        expected_exit: out.exit_code,
        expected_stdout: if raw.compare_stdout {
            Some(out.stdout)
        } else {
            None
        },
        ..provisional
    })
}

const GENERATORS: &[Generator] = &[
    Generator {
        name: "math",
        shard_filename: "gen_40_sql_math.json",
        category: "SQL_MATH",
        priority: "P1",
        id_start: 11000,
        required_capabilities: &["math1"],
        produce: math_functions,
    },
    Generator {
        name: "cast",
        shard_filename: "gen_41_sql_cast.json",
        category: "SQL_CAST",
        priority: "P0",
        id_start: 11200,
        required_capabilities: &[],
        produce: cast_matrix,
    },
    Generator {
        name: "affinity",
        shard_filename: "gen_42_sql_type_affinity.json",
        category: "SQL_TYPE_AFFINITY",
        priority: "P0",
        id_start: 11300,
        required_capabilities: &[],
        produce: type_affinity_matrix,
    },
    Generator {
        name: "string",
        shard_filename: "gen_43_sql_string.json",
        category: "SQL_STRING",
        priority: "P0",
        id_start: 11400,
        required_capabilities: &[],
        produce: string_functions,
    },
    Generator {
        name: "datetime",
        shard_filename: "gen_44_sql_datetime.json",
        category: "SQL_DATETIME",
        priority: "P0",
        id_start: 11500,
        required_capabilities: &[],
        produce: datetime_matrix,
    },
    Generator {
        name: "json_path",
        shard_filename: "gen_45_sql_json_path.json",
        category: "SQL_JSON",
        priority: "P0",
        id_start: 11800,
        required_capabilities: &[],
        produce: json_path_matrix,
    },
    Generator {
        name: "window",
        shard_filename: "gen_46_sql_window.json",
        category: "SQL_WINDOW",
        priority: "P0",
        id_start: 11900,
        required_capabilities: &[],
        produce: window_frames,
    },
    Generator {
        name: "pragma_sweep",
        shard_filename: "gen_47_sql_pragma_sweep.json",
        category: "SQL_PRAGMA",
        priority: "P1",
        id_start: 12000,
        required_capabilities: &[],
        produce: pragma_sweep,
    },
];

pub fn run(repo_root: &Path, sqlite_bin: &str, only: Option<&str>, check: bool) -> Result<()> {
    let cases_dir = repo_root.join("corpus").join("sqlite_parity").join("cases");
    fs::create_dir_all(&cases_dir).context("create corpus/sqlite_parity/cases")?;
    let mut drift = 0usize;
    let mut produced = 0usize;
    for spec in GENERATORS {
        if let Some(name) = only
            && name != spec.name
        {
            continue;
        }
        let cfg = GenConfig {
            category: spec.category,
            priority: spec.priority,
            id_start: spec.id_start,
            required_capabilities: spec.required_capabilities,
        };
        let raw_cases =
            (spec.produce)(&cfg).with_context(|| format!("generator `{}` produce", spec.name))?;
        let mut cases: Vec<Case> = Vec::with_capacity(raw_cases.len());
        for (i, raw) in raw_cases.into_iter().enumerate() {
            let id = cfg.id_start + i as u64;
            cases.push(
                materialize(raw, id, &cfg, sqlite_bin)
                    .with_context(|| format!("materialize case in `{}`", spec.name))?,
            );
        }
        produced += cases.len();
        let target = cases_dir.join(spec.shard_filename);
        let json = serde_json::to_string(&cases)? + "\n";
        if check {
            let on_disk = fs::read_to_string(&target).unwrap_or_default();
            if on_disk != json {
                eprintln!(
                    "DRIFT: generator `{}` differs from on-disk shard {}",
                    spec.name,
                    target.display()
                );
                drift += 1;
            } else {
                eprintln!("ok: {} ({} cases) matches on-disk", spec.name, cases.len());
            }
        } else {
            fs::write(&target, json).with_context(|| format!("write {}", target.display()))?;
            eprintln!(
                "wrote {} ({} cases) -> {}",
                spec.name,
                cases.len(),
                target.display()
            );
        }
    }
    eprintln!("generators produced {produced} cases total");
    if check && drift > 0 {
        bail!("{drift} generators show drift; run `cargo run -p xtask -- generate` to refresh");
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Generators
// ----------------------------------------------------------------------------

fn math_functions(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // 24 math1 functions x 4 inputs. The inputs are chosen so every function
    // produces a finite real value (no domain errors); we capture the actual
    // sqlite3 rendering (including %.17g floating-point format) verbatim.
    let univariate: &[(&str, &[&str])] = &[
        ("acos", &["1.0", "0.0", "-1.0", "0.5"]),
        ("acosh", &["1.0", "2.0", "10.0", "100.0"]),
        ("asin", &["1.0", "0.0", "-1.0", "0.5"]),
        ("asinh", &["1.0", "0.0", "-1.0", "10.0"]),
        ("atan", &["1.0", "0.0", "-1.0", "10.0"]),
        ("atanh", &["0.0", "0.5", "-0.5", "0.9"]),
        ("ceil", &["1.4", "1.6", "-1.4", "-1.6"]),
        ("ceiling", &["1.4", "1.6", "-1.4", "-1.6"]),
        ("cos", &["0.0", "1.0", "3.14159265358979", "-1.0"]),
        ("cosh", &["0.0", "1.0", "-1.0", "2.0"]),
        (
            "degrees",
            &[
                "0.0",
                "3.14159265358979",
                "1.5707963267949",
                "-3.14159265358979",
            ],
        ),
        ("exp", &["0.0", "1.0", "-1.0", "2.0"]),
        ("floor", &["1.4", "1.6", "-1.4", "-1.6"]),
        ("ln", &["1.0", "2.0", "0.5", "10.0"]),
        ("log10", &["1.0", "10.0", "100.0", "0.1"]),
        ("log2", &["1.0", "2.0", "8.0", "0.5"]),
        ("radians", &["0.0", "90.0", "180.0", "-180.0"]),
        (
            "sin",
            &[
                "0.0",
                "1.5707963267949",
                "3.14159265358979",
                "-1.5707963267949",
            ],
        ),
        ("sinh", &["0.0", "1.0", "-1.0", "2.0"]),
        ("sqrt", &["0.0", "1.0", "4.0", "0.25"]),
        (
            "tan",
            &["0.0", "0.7853981633974483", "-0.7853981633974483", "1.0"],
        ),
        ("tanh", &["0.0", "1.0", "-1.0", "10.0"]),
        ("trunc", &["1.4", "1.6", "-1.4", "-1.6"]),
        ("pi", &["<NO_ARG>"]),
    ];
    let mut cases = Vec::new();
    for (func, inputs) in univariate {
        for input in *inputs {
            let stdin = if *input == "<NO_ARG>" {
                format!("SELECT {func}();\n")
            } else {
                format!("SELECT {func}({input});\n")
            };
            let slug = sanitize(input);
            cases.push(RawCase::new(
                format!("MATH_{}_{}", func.to_ascii_uppercase(), slug),
                format!("{func}({input}) sweep cell"),
                stdin,
            ));
        }
    }
    // Binary functions: atan2(y, x), log(b, x), pow(x, y), mod(x, y).
    let binary: &[(&str, &[(&str, &str)])] = &[
        (
            "atan2",
            &[
                ("1.0", "0.0"),
                ("0.0", "1.0"),
                ("1.0", "1.0"),
                ("-1.0", "-1.0"),
            ],
        ),
        ("log", &[("10.0", "100.0"), ("2.0", "8.0"), ("2.0", "1.0")]),
        (
            "pow",
            &[("2.0", "10"), ("3.0", "2"), ("4.0", "0.5"), ("10.0", "-1")],
        ),
        ("power", &[("2.0", "10"), ("3.0", "2")]),
        ("mod", &[("10", "3"), ("10.5", "3.0"), ("-7", "3")]),
    ];
    for (func, pairs) in binary {
        for (a, b) in *pairs {
            cases.push(RawCase::new(
                format!(
                    "MATH_{}_{}_{}",
                    func.to_ascii_uppercase(),
                    sanitize(a),
                    sanitize(b)
                ),
                format!("{func}({a}, {b}) sweep cell"),
                format!("SELECT {func}({a}, {b});\n"),
            ));
        }
    }
    Ok(cases)
}

fn cast_matrix(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // From-value × To-type matrix. Each cell verifies CAST(value AS type)
    // returns the SQLite-canonical conversion for that pair.
    let from_values: &[(&str, &str)] = &[
        ("NULL", "NULL"),
        ("INT_5", "5"),
        ("INT_NEG_3", "-3"),
        ("REAL_3_14", "3.14"),
        ("REAL_0", "0.0"),
        ("TEXT_42", "'42'"),
        ("TEXT_NUM", "'3.14abc'"),
        ("TEXT_BARE", "'abc'"),
        ("BLOB_HEX", "x'3132'"),
    ];
    let to_types = ["INTEGER", "REAL", "TEXT", "BLOB", "NUMERIC"];
    let mut cases = Vec::new();
    for (slug, val) in from_values {
        for to in to_types {
            cases.push(RawCase::new(
                format!("CAST_{}_AS_{}", slug, to),
                format!("CAST({val} AS {to})"),
                format!("SELECT typeof(CAST({val} AS {to})), CAST({val} AS {to});\n"),
            ));
        }
    }
    Ok(cases)
}

fn type_affinity_matrix(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // 5 storage-class affinities × 8 input literal types. For each combination,
    // CREATE a column with the named affinity, INSERT the literal, then SELECT
    // typeof(c), c so the captured output records SQLite's actual storage and
    // rendering decision.
    let affinity_types: &[&str] = &[
        "INTEGER",    // INTEGER affinity
        "BIGINT",     // also INTEGER affinity
        "REAL",       // REAL affinity (only when name contains REAL/FLOA/DOUB)
        "DOUBLE",     // REAL affinity
        "TEXT",       // TEXT affinity
        "VARCHAR(8)", // TEXT affinity (any name containing CHAR/CLOB/TEXT)
        "BLOB",       // BLOB affinity (or empty/declared as BLOB)
        "NUMERIC",    // NUMERIC affinity (catch-all that tries INTEGER then REAL)
    ];
    let literals: &[(&str, &str)] = &[
        ("INT_1", "1"),
        ("INT_BIG", "9223372036854775807"),
        ("REAL_HALF", "0.5"),
        ("REAL_BIG", "1.5e100"),
        ("TEXT_NUM", "'42'"),
        ("TEXT_BARE", "'abc'"),
        ("BLOB_HEX", "x'01ab'"),
        ("NULL_LIT", "NULL"),
    ];
    let mut cases = Vec::new();
    for affinity in affinity_types {
        let affinity_slug = sanitize(affinity);
        for (lit_slug, lit) in literals {
            let stdin = format!(
                "CREATE TABLE t(c {affinity});\nINSERT INTO t VALUES ({lit});\nSELECT typeof(c), c FROM t;\n"
            );
            cases.push(RawCase::new(
                format!("AFFINITY_{}_{}", affinity_slug, lit_slug),
                format!("affinity {affinity} stores {lit}"),
                stdin,
            ));
        }
    }
    Ok(cases)
}

fn string_functions(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // String functions × representative inputs. The cells capture sqlite3's
    // actual output (UTF-8 safe; nothing returns BLOBs).
    let univariate_text: &[(&str, &[&str])] = &[
        ("length", &["'abc'", "''", "'a''b'", "'héllo'"]),
        ("octet_length", &["'abc'", "''", "'héllo'"]),
        ("lower", &["'ABC'", "''", "'AbC'", "'Ｈｅｌｌｏ'"]),
        ("upper", &["'abc'", "''", "'aBc'"]),
        ("ltrim", &["'  hi'", "'  '", "'no-space'"]),
        ("rtrim", &["'hi  '", "'  '", "'no-space'"]),
        ("trim", &["'  hi  '", "'  '"]),
        ("hex", &["'a'", "''", "x'01ab'"]),
        ("quote", &["'a''b'", "1", "NULL", "x'01ab'", "1.5"]),
        ("unicode", &["'a'", "'á'", "''"]),
        ("char", &["97", "65,66,67"]),
        ("zeroblob", &["0", "4"]),
        ("soundex", &["'Robert'", "'Rupert'", "'X'"]),
    ];
    let mut cases = Vec::new();
    for (func, inputs) in univariate_text {
        for input in *inputs {
            cases.push(RawCase::new(
                format!("STRING_{}_{}", func.to_ascii_uppercase(), sanitize(input)),
                format!("{func}({input})"),
                format!("SELECT {func}({input});\n"),
            ));
        }
    }
    // Multi-arg string functions.
    let trio: &[(&str, &str)] = &[
        ("substr", "'abcdef', 2, 3"),
        ("substr", "'abcdef', -2"),
        ("substring", "'abcdef', 2, 3"),
        ("replace", "'aabbaa', 'b', 'X'"),
        ("instr", "'abcdef', 'cd'"),
        ("format", "'%d-%s', 42, 'foo'"),
        ("printf", "'%05d', 7"),
        ("concat", "'a', 'b', 'c'"),
        ("concat_ws", "', ', 'a', 'b', 'c'"),
        ("like", "'a%', 'abc'"),
        ("glob", "'a*', 'abc'"),
        ("unhex", "'0102', ''"),
    ];
    for (func, args) in trio {
        cases.push(RawCase::new(
            format!("STRING_{}_{}", func.to_ascii_uppercase(), sanitize(args)),
            format!("{func}({args})"),
            format!("SELECT {func}({args});\n"),
        ));
    }
    Ok(cases)
}

fn datetime_matrix(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // date/time/datetime/julianday/unixepoch/strftime against representative
    // (anchor, modifier) pairs. Modifiers form their own axis; we sample
    // each documented modifier at least once.
    let anchors: &[(&str, &str)] = &[
        ("EPOCH", "'1970-01-01 00:00:00'"),
        ("Y2K", "'2000-01-01 12:00:00'"),
        ("DATE_2024", "'2024-01-30'"),
        ("UNIX", "1700000000"),
    ];
    let modifiers: &[(&str, &str)] = &[
        ("PLUS_5_DAYS", "'+5 days'"),
        ("MINUS_1_MONTH", "'-1 month'"),
        ("PLUS_1_YEAR", "'+1 year'"),
        ("PLUS_3_HOURS", "'+3 hours'"),
        ("PLUS_30_MINUTES", "'+30 minutes'"),
        ("PLUS_10_SECONDS", "'+10 seconds'"),
        ("START_OF_DAY", "'start of day'"),
        ("START_OF_MONTH", "'start of month'"),
        ("START_OF_YEAR", "'start of year'"),
        ("WEEKDAY_0", "'weekday 0'"),
        ("UTC", "'utc'"),
    ];
    let functions = ["date", "time", "datetime", "julianday", "strftime_iso"];
    let mut cases = Vec::new();
    for (a_slug, a_lit) in anchors {
        for (m_slug, m_lit) in modifiers {
            for func in functions {
                let stdin = match func {
                    "strftime_iso" => {
                        format!("SELECT strftime('%Y-%m-%d %H:%M:%S', {a_lit}, {m_lit});\n")
                    }
                    _ => format!("SELECT {func}({a_lit}, {m_lit});\n"),
                };
                cases.push(RawCase::new(
                    format!(
                        "DATETIME_{}_{}_{}",
                        func.to_ascii_uppercase(),
                        a_slug,
                        m_slug
                    ),
                    format!("{func} on {a_lit} with {m_lit}"),
                    stdin,
                ));
            }
        }
    }
    // No-modifier baseline for each function × anchor.
    for (a_slug, a_lit) in anchors {
        for func in functions {
            let stdin = match func {
                "strftime_iso" => format!("SELECT strftime('%Y-%m-%d', {a_lit});\n"),
                _ => format!("SELECT {func}({a_lit});\n"),
            };
            cases.push(RawCase::new(
                format!("DATETIME_{}_{}_BASE", func.to_ascii_uppercase(), a_slug),
                format!("{func}({a_lit}) baseline"),
                stdin,
            ));
        }
    }
    Ok(cases)
}

fn json_path_matrix(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // JSON1 functions × representative path expressions. The DOC payload is
    // designed to exercise nested objects, nested arrays, key collisions,
    // numeric vs string scalar values.
    let doc = "'{\"a\":{\"b\":[1,2,{\"c\":3}]},\"d\":[null,\"x\",true,4.5],\"e\":\"plain\"}'";
    let paths = [
        "'$'",
        "'$.a'",
        "'$.a.b'",
        "'$.a.b[0]'",
        "'$.a.b[#-1]'",
        "'$.a.b[#-1].c'",
        "'$.d'",
        "'$.d[0]'",
        "'$.d[3]'",
        "'$.e'",
        "'$.missing'",
        "'$.a.b[99]'",
    ];
    // Only path-accepting JSON functions. json_quote and json_valid take a
    // value and (optional) flags, not a path; they're covered elsewhere.
    let path_functions = ["json_extract", "json_type", "json_array_length"];
    let mut cases = Vec::new();
    for func in path_functions {
        for path in paths {
            let path_slug = sanitize(path);
            cases.push(RawCase::new(
                format!("JSON_{}_{}", func.to_ascii_uppercase(), path_slug),
                format!("{func}(doc, {path})"),
                format!("SELECT {func}({doc}, {path});\n"),
            ));
        }
    }
    // Path-less JSON helpers exercised on the same document (3.45-compat).
    cases.push(RawCase::new(
        "JSON_VALID_DOC",
        "json_valid(doc) on the matrix document",
        format!("SELECT json_valid({doc});\n"),
    ));
    cases.push(RawCase::new(
        "JSON_TYPE_DOC_ROOT",
        "json_type(doc) returns 'object' for the root",
        format!("SELECT json_type({doc});\n"),
    ));
    cases.push(RawCase::new(
        "JSON_ARRAY_LENGTH_DOC_ARRAY",
        "json_array_length on the $.d array",
        format!("SELECT json_array_length({doc}, '$.d');\n"),
    ));
    // json_quote on a few interesting scalars (path-less unary form).
    let quote_inputs: &[(&str, &str)] = &[
        ("INT", "5"),
        ("STR", "'a''b'"),
        ("NULL_LIT", "NULL"),
        ("REAL", "3.14"),
    ];
    for (slug, val) in quote_inputs {
        cases.push(RawCase::new(
            format!("JSON_QUOTE_{}", slug),
            format!("json_quote({val})"),
            format!("SELECT json_quote({val});\n"),
        ));
    }
    // The -> and ->> operators (3.38+; SQLite 3.45 has them).
    for path in paths {
        let path_slug = sanitize(path);
        cases.push(RawCase::new(
            format!("JSON_ARROW_{}", path_slug),
            format!("doc -> {path}"),
            format!("SELECT {doc} -> {path};\n"),
        ));
        cases.push(RawCase::new(
            format!("JSON_DARROW_{}", path_slug),
            format!("doc ->> {path}"),
            format!("SELECT {doc} ->> {path};\n"),
        ));
    }
    Ok(cases)
}

fn window_frames(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // Window functions × representative frame specs over a small fixed table.
    let setup = "CREATE TABLE w(g INTEGER, k INTEGER, v REAL);\n\
INSERT INTO w VALUES (1,1,1.0),(1,2,2.0),(1,3,4.0),(1,4,8.0),(2,1,3.0),(2,2,9.0),(2,3,27.0);\n";
    let frames: &[(&str, &str)] = &[
        (
            "ROWS_UNBND_PREC_CURRENT",
            "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW",
        ),
        (
            "ROWS_CURRENT_UNBND_FOLL",
            "ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING",
        ),
        (
            "ROWS_1_PREC_1_FOLL",
            "ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING",
        ),
        (
            "ROWS_2_PREC_CURRENT",
            "ROWS BETWEEN 2 PRECEDING AND CURRENT ROW",
        ),
        (
            "RANGE_UNBND_PREC_CURRENT",
            "RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW",
        ),
        (
            "GROUPS_1_PREC_CURRENT",
            "GROUPS BETWEEN 1 PRECEDING AND CURRENT ROW",
        ),
        (
            "EXCL_CURRENT_ROW",
            "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW EXCLUDE CURRENT ROW",
        ),
        (
            "EXCL_GROUP",
            "ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE GROUP",
        ),
        (
            "EXCL_TIES",
            "ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE TIES",
        ),
        (
            "EXCL_NO_OTHERS",
            "ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE NO OTHERS",
        ),
    ];
    let funcs: &[(&str, &str)] = &[
        ("sum", "v"),
        ("avg", "v"),
        ("count", "*"),
        ("min", "v"),
        ("max", "v"),
        ("row_number", ""),
        ("rank", ""),
        ("dense_rank", ""),
        ("first_value", "v"),
        ("last_value", "v"),
        ("lag", "v"),
        ("lead", "v"),
    ];
    let mut cases = Vec::new();
    for (fslug, finvocation) in funcs {
        for (fslug2, frame_sql) in frames {
            // Rank-family don't take a frame; skip combinations that error.
            let is_rank_family = matches!(
                *fslug,
                "row_number" | "rank" | "dense_rank" | "lag" | "lead"
            );
            if is_rank_family && frame_sql.contains("ROWS")
                || is_rank_family && frame_sql.contains("RANGE")
                || is_rank_family && frame_sql.contains("GROUPS")
            {
                continue; // SQLite parses these but ignores the frame; the
                // captured output is well-defined so we keep some of them.
            }
            let call = if finvocation.is_empty() {
                format!("{fslug}()")
            } else {
                format!("{fslug}({finvocation})")
            };
            let stdin = format!(
                "{setup}SELECT g, k, {call} OVER (PARTITION BY g ORDER BY k {frame_sql}) FROM w ORDER BY g, k;\n"
            );
            cases.push(RawCase::new(
                format!("WIN_{}_{}", fslug.to_ascii_uppercase(), fslug2),
                format!("{call} OVER (PARTITION BY g ORDER BY k {frame_sql})"),
                stdin,
            ));
        }
    }
    // Rank-family with bare OVER (PARTITION BY g ORDER BY k).
    for (fslug, finvocation) in funcs {
        if !matches!(*fslug, "row_number" | "rank" | "dense_rank") {
            continue;
        }
        let call = if finvocation.is_empty() {
            format!("{fslug}()")
        } else {
            format!("{fslug}({finvocation})")
        };
        let stdin = format!(
            "{setup}SELECT g, k, {call} OVER (PARTITION BY g ORDER BY k) FROM w ORDER BY g, k;\n"
        );
        cases.push(RawCase::new(
            format!("WIN_{}_PARTITION", fslug.to_ascii_uppercase()),
            format!("{call} OVER (PARTITION BY g ORDER BY k)"),
            stdin,
        ));
    }
    Ok(cases)
}

fn pragma_sweep(_cfg: &GenConfig) -> Result<Vec<RawCase>> {
    // Pragmas read-only on a fresh in-memory db, plus a few set+read pairs.
    // Designed to bulk up SQL_PRAGMA coverage to P1 across the introspection
    // and perf-knob axes without overlapping with the hand-authored P0 shard.
    let read_only: &[&str] = &[
        "freelist_count",
        "page_count",
        "page_size",
        "max_page_count",
        "cache_size",
        "cache_spill",
        "synchronous",
        "fullfsync",
        "checkpoint_fullfsync",
        "auto_vacuum",
        "temp_store",
        "threads",
        "data_version",
        "encoding",
        "trusted_schema",
        "secure_delete",
        "mmap_size",
        "soft_heap_limit",
        "hard_heap_limit",
        "analysis_limit",
        "automatic_index",
        "reverse_unordered_selects",
        "stats",
        "compile_options",
    ];
    let mut cases = Vec::new();
    for name in read_only {
        cases.push(RawCase::new(
            format!("PRAGMA_{}", name.to_ascii_uppercase()),
            format!("PRAGMA {name} read on fresh :memory:"),
            format!("PRAGMA {name};\n"),
        ));
    }
    // Settable knobs (small integer values that don't break the connection).
    let settable: &[(&str, &str)] = &[
        ("cache_size", "2000"),
        ("page_size", "4096"),
        ("max_page_count", "1073741823"),
        ("mmap_size", "0"),
        ("temp_store", "MEMORY"),
        ("auto_vacuum", "NONE"),
        ("automatic_index", "ON"),
        ("threads", "0"),
        ("analysis_limit", "100"),
        ("synchronous", "NORMAL"),
        ("synchronous", "FULL"),
        ("synchronous", "OFF"),
        ("reverse_unordered_selects", "ON"),
        ("encoding", "'UTF-8'"),
    ];
    for (name, value) in settable {
        cases.push(RawCase::new(
            format!(
                "PRAGMA_SET_{}_{}",
                name.to_ascii_uppercase(),
                sanitize(value)
            ),
            format!("PRAGMA {name}={value} round-trip"),
            format!("PRAGMA {name}={value};\nPRAGMA {name};\n"),
        ));
    }
    Ok(cases)
}

fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    // Collapse repeated underscores and trim leading/trailing ones for cleaner names.
    let mut compact = String::with_capacity(out.len());
    let mut prev_us = false;
    for c in out.chars() {
        if c == '_' {
            if !prev_us {
                compact.push(c);
            }
            prev_us = true;
        } else {
            compact.push(c);
            prev_us = false;
        }
    }
    compact.trim_matches('_').to_owned()
}

// Suppress "produce" function-pointer dead warnings; they're called via the
// GENERATORS table dispatch.
#[allow(dead_code)]
fn _silence_dead(_p: &PathBuf) {}
