//! Beyond-SQLite suite: features.json taxonomy + executable psql-oracle path.
//!
//! The suite has two layers stacked into a single JSONL emission:
//!
//!   1. **Taxonomy emission** (`taxonomy`). For every entry in
//!      `metadata/beyond_sqlite/features.json` we emit a feature-level record
//!      describing rank, owner, proof lane, and the `passing_reference` vs
//!      `manifest_backlog` status. This is the historical contract that
//!      RedlineDB's report parser consumes; it cannot be removed.
//!
//!   2. **Executable oracle** (`oracle`). When
//!      `corpus/beyond_sqlite/generated_manifest.json` lists cases, each case
//!      is run against a Postgres reference (resolved through `engine`) and
//!      the target redlinedb shell. Output is normalized via the per-case
//!      `Normalizer` pipeline before comparison. Cases skip cleanly when
//!      Postgres is unavailable (`REDLINE_TESTING_POSTGRES_URL` unset and the
//!      `pg-embedded` feature off), so `--suite beyond_sqlite` never gates
//!      on Postgres being present.
//!
//! `run()` dispatches to both layers and concatenates their records into the
//! same JSONL file consumed by `report.rs` / `evidence.rs`.

pub mod case;
pub mod engine;
pub mod normalize;
pub mod oracle;
pub mod taxonomy;

pub use taxonomy::{Feature, FeatureStatus, RunConfig, all_features, run};
