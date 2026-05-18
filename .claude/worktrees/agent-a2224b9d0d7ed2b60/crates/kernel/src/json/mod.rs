//! Native JSONB binary format and JSON path bytecode.
//!
//! Lane J2 of phase-10. The wire format is documented in [`wire`]; the
//! encoder and decoder implement deterministic, panic-free conversion to
//! and from `serde_json::Value`. Path expressions compile once into
//! `CompiledPath` (cheaply shareable via `Arc`) and evaluate via a tight
//! interpreter that uses SIMD-friendly key compares for short keys.

pub mod decode;
pub mod encode;
pub mod path_bytecode;
pub mod simd_key;
pub mod wire;

pub use decode::{decode, root_kind};
pub use encode::{encode, encode_blob, encode_blob_value};
pub use path_bytecode::{CompiledPath, Op, compile_path, path_eval, path_kind};
pub use simd_key::{SIMD_KEY_MAX, SIMD_THRESHOLD, key_eq, key_eq_scalar, should_use_simd};
pub use wire::{FORMAT_VERSION, MAGIC, NodeKind};
