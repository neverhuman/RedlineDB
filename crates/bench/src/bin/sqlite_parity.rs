use anyhow::bail;

fn main() -> anyhow::Result<()> {
    bail!(
        "legacy in-tree sqlite_parity binary is disabled: SQLite parity coverage, benchmark, report, and sentinel evidence must be produced only through the pinned neverhuman/redline-testing release artifact"
    )
}
