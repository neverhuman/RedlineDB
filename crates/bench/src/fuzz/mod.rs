//! D7 random-SQL fuzzing for SQLite-parity differential testing.
//!
//! The fuzz harness generates well-formed SQL biased toward the feature
//! surface enumerated in `docs/sqlite-parity.md`, executes each iteration
//! against both `rusqlite::Connection::open_in_memory()` AND a fresh
//! RedlineDB in-memory connection, and asserts equal
//! `(rows-normalized, error-class)` per `normalize.rs`.
//!
//! Entry points:
//!   * [`sqlsmith::generate_stmt`] — produces one statement against a
//!     known schema given a seedable RNG.
//!   * [`normalize::compare_outcomes`] — compares two engine outcomes and
//!     returns a structured `Divergence` (or `Ok`).
//!
//! The integration test (`crates/bench/tests/fuzz_parity.rs`) loops over
//! `REDLINEDB_FUZZ_ITERS` iterations (default 1000) and panics on divergence
//! with the SQL + both outputs pretty-printed for triage. It does not write
//! local SQLite parity evidence.

pub mod normalize;
pub mod sqlsmith;
