//! Unit-level coverage for JSONB encoding, decoding, and path bytecode.
//!
//! These tests focus on the boundary cases listed in Lane J2's spec:
//! varint boundaries, NaN/Inf reals, unicode/emoji text, deep nesting,
//! malformed input rejection, SIMD vs scalar parity, and path bytecode
//! op-by-op coverage.

use redlinedb_kernel::json;
use serde_json::{Value, json};

fn round_trip(v: Value) {
    let bytes = json::encode(&v);
    let back = json::decode(&bytes).expect("decode");
    assert_eq!(back, v);
    let again = json::encode(&back);
    assert_eq!(bytes, again, "stable re-encoding");
}

#[test]
fn null_round_trip() {
    round_trip(Value::Null);
}

#[test]
fn bool_round_trip() {
    round_trip(json!(true));
    round_trip(json!(false));
}

#[test]
fn int_zero_max_min() {
    round_trip(json!(0));
    round_trip(json!(i64::MAX));
    round_trip(json!(i64::MIN));
}

#[test]
fn int_varint_boundaries() {
    // ZigZag-encoded sizes should grow at well-known boundaries.
    for v in [0_i64, 63, 64, -64, 8191, -8192, 1 << 20, -(1 << 20)] {
        let bytes = json::encode(&json!(v));
        let back = json::decode(&bytes).unwrap();
        assert_eq!(back, json!(v));
    }
}

#[test]
fn real_normal_round_trip() {
    round_trip(json!(0.0));
    round_trip(json!(-0.5));
    round_trip(json!(1.5e10));
    round_trip(json!(-1.234_567_89_f64));
}

#[test]
fn real_nan_inf_decode_to_null() {
    // NaN/Inf can't live in serde_json::Number, but the decoder must not panic.
    use json::wire::{FORMAT_VERSION, MAGIC, tag};
    let mut bytes = vec![MAGIC, FORMAT_VERSION, tag::REAL];
    bytes.extend_from_slice(&f64::NAN.to_le_bytes());
    let back = json::decode(&bytes).unwrap();
    assert_eq!(back, Value::Null);

    let mut bytes = vec![MAGIC, FORMAT_VERSION, tag::REAL];
    bytes.extend_from_slice(&f64::INFINITY.to_le_bytes());
    let back = json::decode(&bytes).unwrap();
    assert_eq!(back, Value::Null);
}

#[test]
fn text_empty_ascii_unicode_emoji() {
    round_trip(json!(""));
    round_trip(json!("hello world"));
    round_trip(json!("café — naïve"));
    round_trip(json!("🦀 redline 🚀"));
    round_trip(json!("中文测试"));
}

#[test]
fn array_empty_one_hundred_nested() {
    round_trip(json!([]));
    round_trip(json!([42]));
    let big: Vec<i64> = (0..100).collect();
    round_trip(json!(big));
    round_trip(json!([[[[1, 2], 3], 4], 5]));
}

#[test]
fn object_empty_one_hundred_nested() {
    round_trip(json!({}));
    round_trip(json!({"k": "v"}));
    let mut map = serde_json::Map::new();
    for i in 0..100 {
        map.insert(format!("key{i}"), Value::from(i));
    }
    round_trip(Value::Object(map));
    round_trip(json!({"a": {"b": {"c": {"d": 1}}}}));
}

#[test]
fn blob_round_trip_via_explicit_encoder() {
    let bytes = json::encode_blob_value(&[0u8, 1, 2, 3, 0xFF]);
    // Decoder surfaces blobs as hex-prefixed strings.
    let back = json::decode(&bytes).unwrap();
    assert_eq!(back, json!("0x00010203ff"));
}

#[test]
fn malformed_truncated_text_returns_err() {
    use json::wire::{FORMAT_VERSION, MAGIC, tag};
    // tag::TEXT, varint=10, but only 2 bytes follow.
    let bytes = [MAGIC, FORMAT_VERSION, tag::TEXT, 10, b'h', b'i'];
    assert!(json::decode(&bytes).is_err());
}

#[test]
fn malformed_unknown_tag_returns_err() {
    use json::wire::{FORMAT_VERSION, MAGIC};
    let bytes = [MAGIC, FORMAT_VERSION, 0x0F];
    assert!(json::decode(&bytes).is_err());
}

#[test]
fn malformed_bad_magic_returns_err() {
    let bytes = [0x42, json::FORMAT_VERSION, 0x00];
    assert!(json::decode(&bytes).is_err());
}

#[test]
fn malformed_bad_version_returns_err() {
    let bytes = [json::MAGIC, json::FORMAT_VERSION + 9, 0x00];
    assert!(json::decode(&bytes).is_err());
}

#[test]
fn malformed_random_bytes_never_panic() {
    for seed in 0u64..64 {
        let mut buf = Vec::new();
        for j in 0..32 {
            buf.push(((seed.wrapping_mul(6364136223846793005)) >> (j % 32)) as u8);
        }
        let _ = json::decode(&buf);
    }
}

#[test]
fn path_compile_root() {
    let p = json::compile_path("$").unwrap();
    assert_eq!(p.ops, vec![json::Op::Root, json::Op::Return]);
}

#[test]
fn path_compile_object_key() {
    let p = json::compile_path("$.foo").unwrap();
    assert!(matches!(p.ops[1], json::Op::LoadObjKey(_)));
}

#[test]
fn path_compile_array_idx() {
    let p = json::compile_path("$[7]").unwrap();
    assert_eq!(p.ops[1], json::Op::LoadArrIdx(7));
}

#[test]
fn path_compile_quoted_key() {
    let p = json::compile_path("$.\"hello world\"").unwrap();
    assert_eq!(p.literals, vec!["hello world".to_string()]);
}

#[test]
fn path_compile_errors() {
    for bad in ["", "foo", "$.", "$[", "$[abc]", "$.\"unterminated", "$.@"] {
        assert!(json::compile_path(bad).is_err(), "should reject {bad:?}");
    }
}

#[test]
fn path_eval_happy() {
    let v = json!({"users": [{"name": "ada"}, {"name": "lin"}]});
    let bytes = json::encode(&v);
    let p = json::compile_path("$.users[1].name").unwrap();
    let slice = json::path_eval(&bytes, &p).unwrap().unwrap();
    let prefixed = {
        let mut b = vec![json::MAGIC, json::FORMAT_VERSION];
        b.extend_from_slice(slice);
        b
    };
    assert_eq!(json::decode(&prefixed).unwrap(), json!("lin"));
}

#[test]
fn path_eval_missing_key() {
    let v = json!({"a": 1});
    let bytes = json::encode(&v);
    let p = json::compile_path("$.b").unwrap();
    assert!(json::path_eval(&bytes, &p).unwrap().is_none());
}

#[test]
fn path_eval_array_oor() {
    let v = json!([1, 2, 3]);
    let bytes = json::encode(&v);
    let p = json::compile_path("$[99]").unwrap();
    assert!(json::path_eval(&bytes, &p).unwrap().is_none());
}

#[test]
fn path_eval_type_mismatch_returns_none() {
    let v = json!({"a": 1});
    let bytes = json::encode(&v);
    let p = json::compile_path("$[0]").unwrap();
    assert!(json::path_eval(&bytes, &p).unwrap().is_none());
}

#[test]
fn path_eval_root_returns_whole_doc() {
    let v = json!({"x": 1, "y": 2});
    let bytes = json::encode(&v);
    let p = json::compile_path("$").unwrap();
    let slice = json::path_eval(&bytes, &p).unwrap().unwrap();
    let prefixed = {
        let mut b = vec![json::MAGIC, json::FORMAT_VERSION];
        b.extend_from_slice(slice);
        b
    };
    assert_eq!(json::decode(&prefixed).unwrap(), v);
}

#[test]
fn simd_key_compare_matches_scalar_random() {
    // 1000 random key/object pairs, mixed lengths, comparing SIMD against
    // scalar.
    fn xorshift(s: &mut u64) -> u64 {
        let mut x = *s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s = x;
        x
    }
    let mut s = 0xA1B2_C3D4_E5F6_0708_u64;
    for _ in 0..1000 {
        let len_a = (xorshift(&mut s) % 24) as usize;
        let len_b = (xorshift(&mut s) % 24) as usize;
        let mut a = Vec::with_capacity(len_a);
        let mut b = Vec::with_capacity(len_b);
        for _ in 0..len_a {
            a.push((xorshift(&mut s) & 0xFF) as u8);
        }
        for _ in 0..len_b {
            b.push((xorshift(&mut s) & 0xFF) as u8);
        }
        // Ensure parity in both directions.
        assert_eq!(json::key_eq(&a, &b), json::key_eq_scalar(&a, &b));
        // Self-equality always true.
        assert!(json::key_eq(&a, &a));
        assert!(json::key_eq(&b, &b));
    }
}

#[test]
fn simd_threshold_matrix() {
    assert!(!json::should_use_simd(0, 4));
    assert!(!json::should_use_simd(3, 4));
    assert!(json::should_use_simd(4, 0));
    assert!(json::should_use_simd(8, 16));
    assert!(!json::should_use_simd(8, 17));
}

#[test]
fn cached_compiled_path_is_arc_shareable() {
    let p1 = json::compile_path("$.x.y").unwrap();
    let p2 = std::sync::Arc::clone(&p1);
    assert_eq!(p1.ops, p2.ops);
}

#[test]
fn deterministic_encoding_across_runs() {
    let v = json!({"a": 1, "b": [true, null, "x"], "c": {"d": 2}});
    let a = json::encode(&v);
    let b = json::encode(&v);
    assert_eq!(a, b);
}
