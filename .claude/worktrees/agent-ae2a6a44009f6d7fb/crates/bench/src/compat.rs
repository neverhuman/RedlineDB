use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::config::{CompatArgs, DurabilityKind, EngineKind, RunSpec};
use crate::engine::{self, BenchEngine, CellValue};

#[derive(Debug, Serialize)]
pub struct CompatReport {
    pub files: usize,
    pub cases: usize,
    pub failures: Vec<String>,
}

enum Case {
    Statement(String),
    Query {
        sql: String,
        expected: Vec<Vec<CellValue>>,
    },
}

pub fn run_suite(args: &CompatArgs) -> Result<CompatReport> {
    let files = collect_cases(&args.test_dir)?;
    let mut report = CompatReport {
        files: files.len(),
        cases: 0,
        failures: Vec::new(),
    };
    for (path, cases) in files {
        report.cases += cases.len();
        let actual = run_file(args.engine.expand(), &cases, args.seed)
            .with_context(|| format!("run compat file {}", path.display()));
        if let Err(err) = actual {
            report.failures.push(format!("{err:#}"));
        }
    }
    Ok(report)
}

fn run_file(engines: &[EngineKind], cases: &[Case], seed: u64) -> Result<()> {
    let mut outputs = Vec::new();
    for &engine_kind in engines {
        let spec = RunSpec {
            engine: engine_kind,
            workload: crate::config::WorkloadKind::PointReadPk,
            durability: DurabilityKind::Strict,
            threads: 1,
            rows: 32,
            duration: std::time::Duration::from_secs(1),
            cache_bytes: 8 * 1024 * 1024,
            seed,
            base_dir: std::env::temp_dir().join("redlinedb-bench-compat"),
        };
        let db_dir = spec.base_dir.join(format!("{engine_kind:?}-{seed}"));
        let _ = fs::remove_dir_all(&db_dir);
        let engine = engine::open(&spec, &db_dir)?;
        engine.setup_schema()?;
        outputs.push(run_cases(engine.as_ref(), cases)?);
    }
    if outputs.windows(2).any(|pair| pair[0] != pair[1]) {
        bail!("compat output mismatch between engines");
    }
    Ok(())
}

fn run_cases(engine: &dyn BenchEngine, cases: &[Case]) -> Result<Vec<Vec<Vec<CellValue>>>> {
    let mut conn = engine.connect(0)?;
    let mut outputs = Vec::new();
    for case in cases {
        match case {
            Case::Statement(sql) => {
                let _ = conn.execute(sql, &[])?;
                outputs.push(Vec::new());
            }
            Case::Query { sql, expected } => {
                let actual = conn.query_all(sql, &[])?;
                if &actual != expected {
                    bail!("query mismatch for `{sql}`");
                }
                outputs.push(actual);
            }
        }
    }
    Ok(outputs)
}

fn collect_cases(dir: &Path) -> Result<Vec<(std::path::PathBuf, Vec<Case>)>> {
    let mut files = Vec::new();
    collect_cases_recursive(dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        bail!("no .sqlt files found in {}", dir.display());
    }
    Ok(files)
}

fn collect_cases_recursive(
    dir: &Path,
    files: &mut Vec<(std::path::PathBuf, Vec<Case>)>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_cases_recursive(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "sqlt") {
            files.push((path.clone(), parse_cases(&fs::read_to_string(&path)?)?));
        }
    }
    Ok(())
}

fn parse_cases(input: &str) -> Result<Vec<Case>> {
    let chunks = input
        .split("\n\n")
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty());
    let mut cases = Vec::new();
    for chunk in chunks {
        let lines: Vec<_> = chunk.lines().collect();
        if lines.is_empty() {
            continue;
        }
        match lines[0] {
            "statement ok" => cases.push(Case::Statement(lines[1..].join("\n"))),
            "query" => {
                let joined = lines[1..].join("\n");
                let (sql, expected) = joined
                    .split_once("\n----\n")
                    .ok_or_else(|| anyhow::anyhow!("query case missing `----` separator"))?;
                let expected = expected
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(parse_expected_row)
                    .collect::<Result<Vec<_>>>()?;
                cases.push(Case::Query {
                    sql: sql.to_owned(),
                    expected,
                });
            }
            other => bail!("unknown compat directive `{other}`"),
        }
    }
    Ok(cases)
}

fn parse_expected_row(line: &str) -> Result<Vec<CellValue>> {
    line.split('|')
        .map(|cell| match cell {
            "NULL" => Ok(CellValue::Null),
            value if let Ok(parsed) = value.parse::<i64>() => Ok(CellValue::Integer(parsed)),
            value => Ok(CellValue::Text(value.to_owned())),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parser_reads_statement_and_query_cases() {
        let input = "statement ok\nCREATE TABLE t(id INTEGER)\n\nquery\nSELECT 1\n----\n1\n";
        let cases = parse_cases(input).expect("cases");
        assert_eq!(cases.len(), 2);
    }

    #[test]
    fn collect_cases_recurses_into_nested_directories() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let nested = tmp.path().join("orm");
        fs::create_dir_all(&nested).expect("nested dir");
        fs::write(
            nested.join("migration.sqlt"),
            "statement ok\nCREATE TABLE t(id INTEGER)\n",
        )
        .expect("write sqlt");

        let files = collect_cases(tmp.path()).expect("files");
        assert_eq!(files.len(), 1);
        assert!(files[0].0.ends_with("migration.sqlt"));
    }
}
