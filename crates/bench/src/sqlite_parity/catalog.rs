use anyhow::{Context, Result};

use super::case::Case;

const MANIFEST: &str = include_str!("../../sqlite_parity/generated_manifest.json");

pub fn all_cases() -> Result<Vec<Case>> {
    serde_json::from_str(MANIFEST).context("parse sqlite parity generated_manifest.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sqlite_parity::case::Priority;

    #[test]
    fn generated_catalog_has_planned_size_and_ranges() {
        let cases = all_cases().expect("sqlite parity manifest");
        assert_eq!(cases.len(), 1127);
        assert_eq!(cases[0].display_id(), "00001");
        assert_eq!(cases[1126].display_id(), "01127");
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.priority == Priority::P0)
                .count(),
            130
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.priority == Priority::P1)
                .count(),
            579
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.priority == Priority::P2)
                .count(),
            370
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.priority == Priority::P3)
                .count(),
            24
        );
    }
}
