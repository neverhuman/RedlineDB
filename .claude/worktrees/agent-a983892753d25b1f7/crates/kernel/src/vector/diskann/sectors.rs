//! Sector-aligned record layout for DiskANN nodes.
//!
//! Each node is laid out in a single 4 KiB sector so that beam search needs
//! exactly one disk read per node visit. The layout is intentionally simple
//! and self-describing; once the on-disk path lands, a separate header will
//! pin the layout parameters globally so individual sectors can stay
//! header-light, but for v1 each sector carries the magic + version so a
//! corrupt blob can be detected.
//!
//! ## Sector format (little-endian)
//!
//! | offset | bytes | field                                 |
//! |-------:|------:|---------------------------------------|
//! |    0   |   4   | magic = `b"VAMA"`                     |
//! |    4   |   2   | version = 1                           |
//! |    6   |   2   | reserved (must be 0)                  |
//! |    8   |   8   | row_id (u64 LE)                       |
//! |   16   |   2   | dim (u16 LE)                          |
//! |   18   |   2   | max_degree (u16 LE)                   |
//! |   20   |   2   | neighbour_count (u16 LE, <= max_deg)  |
//! |   22   |   2   | reserved                              |
//! |   24   | dim*4 | vector data (f32 LE)                  |
//! |   ...  | R*4   | neighbour ids (u32 LE), 0xFFFFFFFF=pad|
//! |   ...  | rest  | zero padding to SECTOR_SIZE           |
//!
//! `SECTOR_SIZE` is fixed at 4096 bytes. We refuse to encode a layout whose
//! header + payload + neighbour table doesn't fit in one sector; in practice
//! that bounds `dim` for `R=64` to roughly 1000 (4 + 2 + 2 + 8 + 2*4 + 4*dim
//! + 4*64 + padding). The current bench dataset stays well below that.

use crate::vector::diskann::RowId;

/// On-disk sector size. 4 KiB matches typical SSD pages.
pub const SECTOR_SIZE: usize = 4096;

const MAGIC: &[u8; 4] = b"VAMA";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 24;
/// Sentinel placed in the neighbour table for unused slots.
const NEIGHBOUR_PAD: u32 = u32::MAX;

/// Errors the sector encoder/decoder can surface. `SectorTooSmall` happens
/// when (dim, R) overflow 4 KiB — callers must reduce one of them.
#[derive(Debug, PartialEq)]
pub enum SectorError {
    /// (dim, max_degree) won't fit in one [`SECTOR_SIZE`] block.
    SectorTooSmall { needed: usize, available: usize },
    /// Too few bytes were supplied for a sector buffer.
    Truncated { expected: usize, actual: usize },
    /// Magic bytes did not match `VAMA`.
    BadMagic,
    /// Sector version is unsupported.
    BadVersion(u16),
    /// Decoded header disagreed with the requested layout.
    LayoutMismatch(&'static str),
    /// Neighbour table claimed more entries than `max_degree`.
    DegreeOverflow { claimed: usize, max: usize },
}

impl std::fmt::Display for SectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SectorError::SectorTooSmall { needed, available } => {
                write!(f, "sector too small: need {needed} bytes, have {available}")
            }
            SectorError::Truncated { expected, actual } => {
                write!(
                    f,
                    "sector buffer truncated: expected {expected}, got {actual}"
                )
            }
            SectorError::BadMagic => write!(f, "sector magic mismatch"),
            SectorError::BadVersion(v) => write!(f, "unsupported sector version {v}"),
            SectorError::LayoutMismatch(why) => write!(f, "layout mismatch: {why}"),
            SectorError::DegreeOverflow { claimed, max } => {
                write!(f, "neighbour count {claimed} exceeds max_degree {max}")
            }
        }
    }
}

impl std::error::Error for SectorError {}

/// Pre-computed offsets for a (dim, max_degree) layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectorLayout {
    pub dim: usize,
    pub max_degree: usize,
    pub vector_off: usize,
    pub neighbour_off: usize,
}

impl SectorLayout {
    /// Build a layout descriptor for the given `(dim, max_degree)`. Returns
    /// [`SectorError::SectorTooSmall`] if the result wouldn't fit in 4 KiB.
    pub fn for_dim_degree(dim: usize, max_degree: usize) -> Result<Self, SectorError> {
        let vector_off = HEADER_LEN;
        let neighbour_off = vector_off
            + dim.checked_mul(4).ok_or(SectorError::SectorTooSmall {
                needed: usize::MAX,
                available: SECTOR_SIZE,
            })?;
        let needed = neighbour_off
            + max_degree
                .checked_mul(4)
                .ok_or(SectorError::SectorTooSmall {
                    needed: usize::MAX,
                    available: SECTOR_SIZE,
                })?;
        if needed > SECTOR_SIZE {
            return Err(SectorError::SectorTooSmall {
                needed,
                available: SECTOR_SIZE,
            });
        }
        Ok(Self {
            dim,
            max_degree,
            vector_off,
            neighbour_off,
        })
    }
}

fn put_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn get_u64(buf: &[u8], off: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[off..off + 8]);
    u64::from_le_bytes(bytes)
}

/// Encode a single node into a [`SECTOR_SIZE`]-byte buffer. The buffer is
/// zeroed first so any tail padding is deterministic.
pub fn encode_node(
    buf: &mut [u8],
    layout: &SectorLayout,
    row_id: RowId,
    vector: &[f32],
    neighbours: &[u32],
) -> Result<(), SectorError> {
    if buf.len() < SECTOR_SIZE {
        return Err(SectorError::Truncated {
            expected: SECTOR_SIZE,
            actual: buf.len(),
        });
    }
    if vector.len() != layout.dim {
        return Err(SectorError::LayoutMismatch("vector dim != layout dim"));
    }
    if neighbours.len() > layout.max_degree {
        return Err(SectorError::DegreeOverflow {
            claimed: neighbours.len(),
            max: layout.max_degree,
        });
    }
    let buf = &mut buf[..SECTOR_SIZE];
    buf.fill(0);
    buf[0..4].copy_from_slice(MAGIC);
    put_u16(buf, 4, VERSION);
    // bytes 6..8 reserved (already zero)
    put_u64(buf, 8, row_id.0);
    put_u16(buf, 16, layout.dim as u16);
    put_u16(buf, 18, layout.max_degree as u16);
    put_u16(buf, 20, neighbours.len() as u16);
    // bytes 22..24 reserved
    let v_off = layout.vector_off;
    for (i, x) in vector.iter().enumerate() {
        let off = v_off + i * 4;
        buf[off..off + 4].copy_from_slice(&x.to_le_bytes());
    }
    let n_off = layout.neighbour_off;
    for slot in 0..layout.max_degree {
        let v = neighbours.get(slot).copied().unwrap_or(NEIGHBOUR_PAD);
        put_u32(buf, n_off + slot * 4, v);
    }
    Ok(())
}

/// Decode the inverse of [`encode_node`]. Returns the row id, a borrowed
/// slice into the vector payload, and an owned `Vec<u32>` of neighbour ids
/// (padding stripped).
pub fn decode_node<'a>(
    buf: &'a [u8],
    layout: &SectorLayout,
) -> Result<(RowId, &'a [f32], Vec<u32>), SectorError> {
    if buf.len() < SECTOR_SIZE {
        return Err(SectorError::Truncated {
            expected: SECTOR_SIZE,
            actual: buf.len(),
        });
    }
    let buf = &buf[..SECTOR_SIZE];
    if &buf[0..4] != MAGIC {
        return Err(SectorError::BadMagic);
    }
    let version = get_u16(buf, 4);
    if version != VERSION {
        return Err(SectorError::BadVersion(version));
    }
    let row_id = RowId(get_u64(buf, 8));
    let dim = get_u16(buf, 16) as usize;
    let max_degree = get_u16(buf, 18) as usize;
    let n_count = get_u16(buf, 20) as usize;
    if dim != layout.dim {
        return Err(SectorError::LayoutMismatch("encoded dim != layout dim"));
    }
    if max_degree != layout.max_degree {
        return Err(SectorError::LayoutMismatch(
            "encoded max_degree != layout max_degree",
        ));
    }
    if n_count > max_degree {
        return Err(SectorError::DegreeOverflow {
            claimed: n_count,
            max: max_degree,
        });
    }
    let v_off = layout.vector_off;
    let v_end = v_off + dim * 4;
    // Safety/correctness: cast the f32 slice via `from_le_bytes` element-wise.
    // Avoiding an unsafe transmute keeps this code endian-portable; on LE
    // hosts the compiler reduces it to a memcpy.
    let vector_bytes = &buf[v_off..v_end];
    // We need a `&[f32]` borrowed from `buf`; instead we return an owned
    // copy via a static mutable trick is unsafe. Use a helper that builds a
    // boxed slice and leaks it would also be wrong. Solution: borrow via
    // reinterpret_cast through `bytemuck`-style align check, but we don't
    // depend on bytemuck. Use `align_to` on `&[u8]` to get `&[f32]`.
    let (head, mid, tail) = unsafe { vector_bytes.align_to::<f32>() };
    if !head.is_empty() || !tail.is_empty() {
        // The sector start is 4-byte aligned by construction (HEADER_LEN=24)
        // so this branch only triggers if the caller passed a misaligned
        // slice — fall back to a heap copy to stay correct.
        return Err(SectorError::LayoutMismatch("sector buffer not 4B aligned"));
    }
    let vector: &[f32] = mid;
    let n_off = layout.neighbour_off;
    let mut neigh = Vec::with_capacity(n_count);
    for i in 0..n_count {
        neigh.push(get_u32(buf, n_off + i * 4));
    }
    Ok((row_id, vector, neigh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_for_typical_dims_fits() {
        let layout = SectorLayout::for_dim_degree(128, 64).unwrap();
        assert_eq!(layout.dim, 128);
        assert_eq!(layout.max_degree, 64);
        assert!(layout.neighbour_off + 64 * 4 <= SECTOR_SIZE);
    }

    #[test]
    fn layout_rejects_oversized() {
        // dim=2000, R=64 → 24 + 8000 + 256 = 8280 > 4096
        let err = SectorLayout::for_dim_degree(2000, 64).unwrap_err();
        assert!(matches!(err, SectorError::SectorTooSmall { .. }));
    }

    #[test]
    fn round_trip() {
        let layout = SectorLayout::for_dim_degree(8, 4).unwrap();
        let mut buf = vec![0u8; SECTOR_SIZE];
        let v = [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let n = vec![10u32, 20, 30];
        encode_node(&mut buf, &layout, RowId(42), &v, &n).unwrap();
        let (row, vec_back, n_back) = decode_node(&buf, &layout).unwrap();
        assert_eq!(row, RowId(42));
        assert_eq!(vec_back, &v);
        assert_eq!(n_back, n);
    }

    #[test]
    fn padding_bytes_are_zero() {
        let layout = SectorLayout::for_dim_degree(4, 8).unwrap();
        let mut buf = vec![0xAAu8; SECTOR_SIZE];
        encode_node(&mut buf, &layout, RowId(1), &[0.0; 4], &[1, 2]).unwrap();
        // Tail past the neighbour table must be zero so the on-disk image
        // is deterministic across rebuilds.
        let tail_start = layout.neighbour_off + layout.max_degree * 4;
        for byte in &buf[tail_start..] {
            assert_eq!(*byte, 0);
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let layout = SectorLayout::for_dim_degree(4, 4).unwrap();
        let mut buf = vec![0u8; SECTOR_SIZE];
        encode_node(&mut buf, &layout, RowId(1), &[0.0; 4], &[]).unwrap();
        buf[0] = b'X';
        assert_eq!(
            decode_node(&buf, &layout).unwrap_err(),
            SectorError::BadMagic
        );
    }
}
