use anyhow::Result;

use crate::beyond_sqlite;
use crate::report::{self, JankuraiCompareOptions, ReportOptions, SentinelOptions};
use crate::sqlite_parity;

use super::args::{JankuraiCompareArgs, ListArgs, ListFormat, ReportArgs, SentinelArgs, Suite};

pub(crate) fn report(args: ReportArgs) -> Result<()> {
    report::generate(ReportOptions {
        suite: args.suite.as_str().to_owned(),
        input: args.input,
        official_evidence: args.official_evidence,
        local_diagnostics: args.local_diagnostics,
        out_dir: args.out_dir,
        readme: args.readme,
        plot: args.plot,
        ksloc_plot: args.ksloc_plot,
        performance_histogram_plot: args.performance_histogram_plot,
        median_test_performance_plot: args.median_test_performance_plot,
        jankurai_score: args.jankurai_score,
        jankurai_comparison: args.jankurai_comparison,
        jankurai_comparison_plot: args.jankurai_comparison_plot,
        jankurai_score_plot: args.jankurai_score_plot,
        code_shape_plot: args.code_shape_plot,
        updated_date: args.updated_date,
        expected_repetitions: args.expected_repetitions,
        expected_warmup: args.expected_warmup,
        check: args.check,
    })
}

pub(crate) fn list(args: ListArgs) -> Result<()> {
    if matches!(args.suite, Suite::BeyondSqlite) {
        return list_beyond_sqlite(args.format);
    }
    let selected = match args.suite {
        Suite::All | Suite::SqliteParity | Suite::Memory => sqlite_parity::all_cases()?,
        Suite::RqlPhase1 => sqlite_parity::rql_phase1_cases()?,
        Suite::BeyondSqlite => unreachable!("handled above"),
    };
    match args.format {
        ListFormat::Text => {
            for case in selected {
                println!(
                    "{} {} {} {} {}",
                    case.display_id(),
                    case.priority,
                    case.profile,
                    case.category,
                    case.name
                );
            }
        }
        ListFormat::Markdown => {
            println!("# SQLite Parity Test Index\n");
            println!("| ID | Priority | Profile | Category | Name | Case file |");
            println!("| --- | --- | --- | --- | --- | --- |");
            for case in selected {
                println!(
                    "| {} | {} | {} | {} | {} | `{}` |",
                    case.display_id(),
                    case.priority,
                    case.profile,
                    case.category,
                    case.name,
                    case.case_file_name()
                );
            }
        }
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&selected)?);
        }
    }
    Ok(())
}

fn list_beyond_sqlite(format: ListFormat) -> Result<()> {
    let features = beyond_sqlite::all_features()?;
    match format {
        ListFormat::Text => {
            for feature in features {
                println!(
                    "BEYOND-{:03} {} {} {}",
                    feature.rank,
                    feature.status_string(),
                    feature.proof_lane,
                    feature.title
                );
            }
        }
        ListFormat::Markdown => {
            println!("# Beyond-SQLite Feature Index\n");
            println!("| ID | Status | Owner | Proof lane | Title |");
            println!("| --- | --- | --- | --- | --- |");
            for feature in features {
                println!(
                    "| BEYOND-{:03} | {} | {} | {} | {} |",
                    feature.rank,
                    feature.status_string(),
                    feature.owner,
                    feature.proof_lane,
                    feature.title
                );
            }
        }
        ListFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&features)?);
        }
    }
    Ok(())
}

pub(crate) fn jankurai_compare(args: JankuraiCompareArgs) -> Result<()> {
    report::jankurai_compare(JankuraiCompareOptions {
        redlinedb_score: args.redlinedb_score,
        sqlite_score: args.sqlite_score,
        sqlite_ref: args.sqlite_ref,
        updated_date: args.updated_date,
        json: args.json,
        csv: args.csv,
        check: args.check,
    })
}

pub(crate) fn sentinel(args: SentinelArgs) -> Result<()> {
    report::sentinel(SentinelOptions {
        input: args.input,
        ceiling_ns: args.ceiling_ns,
        enforce: args.enforce,
    })
}

impl beyond_sqlite::Feature {
    pub(crate) fn status_string(&self) -> &'static str {
        match self.status {
            beyond_sqlite::FeatureStatus::ManifestBacklog => "manifest_backlog",
            beyond_sqlite::FeatureStatus::PassingReference => "passing_reference",
        }
    }
}
