pub(crate) mod args;
mod cmds;
pub(crate) mod run;

pub use args::Cli;

use anyhow::Result;
use args::CommandKind;

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CommandKind::Run(args) => run::run_suite(args),
        CommandKind::MajorGate(args) => cmds::major_gate(args),
        CommandKind::Report(args) => cmds::report(args),
        CommandKind::List(args) => cmds::list(args),
        CommandKind::JankuraiCompare(args) => cmds::jankurai_compare(args),
        CommandKind::Sentinel(args) => cmds::sentinel(args),
        CommandKind::Version => {
            println!("redline-testing {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
