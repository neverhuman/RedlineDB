//! RedlineDB domain crate.
//!
//! Hosts cross-crate domain types that are policy-free and dependency-light.
//! The first inhabitant is [`DomainError`] — a typed exception surface that
//! carries enough context for the next agent to triage a failure without
//! rereading the source. See `docs/audit-rubric.md` and
//! `docs/language-bad-behavior.md` for the conventions this surface enforces.

pub mod error;

pub use error::DomainError;
