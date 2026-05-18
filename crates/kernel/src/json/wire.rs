//! RedlineDB JSONB wire-format façade.
//!
//! The implementation lives in `wire/common.rs` and `wire/iter.rs`; this
//! module preserves the public API expected by the encoder, decoder, and path
//! interpreter while keeping the authored surface small.

#[path = "wire/common.rs"]
mod common;
#[path = "wire/iter.rs"]
mod iter;
#[cfg(test)]
#[path = "wire/tests.rs"]
mod tests;

pub use common::{
    FORMAT_VERSION, MAGIC, MAX_VARINT_LEN, NodeKind, PREAMBLE_LEN, node_span, parse_preamble,
    read_bytes_payload, read_tag, read_varint, tag, write_varint, zigzag_decode, zigzag_encode,
};
pub use iter::{ArrayIter, ObjectIter};
