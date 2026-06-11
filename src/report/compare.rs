use std::collections::BTreeMap;
use std::fs;

use anyhow::{bail, Context, Result};

use super::render::parse_raw_records;
use super::types::{JankuraiCompareOptions, SentinelOptions};
use super::utils::{is_measured, parse_score, verify_text, write_text};

pub fn jankurai_compare(options: JankuraiCompareOptions) -> Result<()> {
    let redlinedb = parse_score(&options.redlinedb_score)?;
    let sqlite = parse_score(&options.sqlite_score)?;
    let diff = redlinedb.score as i64 - sqlite.score as i64;
    let comparison = serde_json::json!({
        "generated_by": "redline-testing jankurai-compare",
        "updated_date": options.updated_date,
        "sqlite_ref": options.sqlite_ref,
        "redlinedb_score": redlinedb.score,
        "sqlite_score": sqlite.score,
        "score_delta": diff,
        "redlinedb_status": redlinedb.status,
        "sqlite_status": sqlite.status,
    });
    let csv = format!(
        "repo,score,status\nredlinedb,{},{}\nsqlite,{},{}\n",
        redlinedb.score, redlinedb.status, sqlite.score, sqlite.status
    );

    if options.check {
        verify_text(
            &options.json,
            &format!("{}\n", serde_json::to_string_pretty(&comparison)?),
        )?;
        verify_text(&options.csv, &csv)?;
        return Ok(());
    }

    write_text(
        &options.json,
        &format!("{}\n", serde_json::to_string_pretty(&comparison)?),
    )?;
    write_text(&options.csv, &csv)?;
    Ok(())
}

pub fn sentinel(options: SentinelOptions) -> Result<()> {
    let ceilings = options
        .ceiling_ns
        .iter()
        .map(|entry| {
            let (case_id, value) = entry
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("invalid ceiling entry `{entry}`"))?;
            let value = value
                .parse::<u128>()
                .with_context(|| format!("parse ceiling value for {case_id}"))?;
            Ok((case_id.to_owned(), value))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let raw_text = fs::read_to_string(&options.input)
        .with_context(|| format!("read sentinel input {}", options.input.display()))?;
    let mut violations = Vec::new();
    for record in parse_raw_records(&raw_text)? {
        if !is_measured(&record) {
            continue;
        }
        if let Some(ceiling) = ceilings.get(&record.case_id)
            && record.target_elapsed_ns > *ceiling
        {
            violations.push(format!(
                "{}: {} ns > ceiling {} ns",
                record.case_id, record.target_elapsed_ns, ceiling
            ));
        }
    }
    if violations.is_empty() {
        if options.enforce {
            eprintln!("sqlite parity sentinel passed");
        }
        return Ok(());
    }
    let message = violations.join("; ");
    if options.enforce {
        bail!("{message}");
    }
    eprintln!("{message}");
    Ok(())
}
