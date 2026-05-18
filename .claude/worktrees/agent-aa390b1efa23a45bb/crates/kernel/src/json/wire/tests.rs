use super::common::{FORMAT_VERSION, MAGIC, PREAMBLE_LEN, parse_preamble, write_varint};
use super::common::{read_varint, zigzag_decode, zigzag_encode};

#[test]
fn varint_round_trip_small() {
    for v in [0_u64, 1, 127, 128, 255, 16_383, 16_384, u64::MAX] {
        let mut buf = Vec::new();
        write_varint(&mut buf, v);
        let (decoded, len) = read_varint(&buf, 0).unwrap();
        assert_eq!(decoded, v);
        assert_eq!(len, buf.len());
    }
}

#[test]
fn varint_truncated_returns_err() {
    let buf = [0x80_u8];
    assert!(read_varint(&buf, 0).is_err());
}

#[test]
fn varint_overlong_returns_err() {
    let buf = [0x80_u8; 11];
    assert!(read_varint(&buf, 0).is_err());
}

#[test]
fn zigzag_round_trips_extremes() {
    for v in [0_i64, 1, -1, i64::MAX, i64::MIN, 1234567, -7654321] {
        assert_eq!(zigzag_decode(zigzag_encode(v)), v);
    }
}

#[test]
fn parse_preamble_rejects_garbage() {
    assert!(parse_preamble(&[]).is_err());
    assert!(parse_preamble(&[0x00, 0x01]).is_err());
    assert!(parse_preamble(&[MAGIC, 0xFF]).is_err());
    let off = parse_preamble(&[MAGIC, FORMAT_VERSION]).unwrap();
    assert_eq!(off, PREAMBLE_LEN);
}
