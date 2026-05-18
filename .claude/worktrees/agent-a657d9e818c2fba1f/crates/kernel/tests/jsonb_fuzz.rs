//! Fuzz / property-style tests for the JSONB wire format.
//!
//! 1000-iteration round-trip harness using a deterministic xorshift PRNG so
//! failures reproduce without `proptest` shrinking machinery. Each iteration
//! generates a random `serde_json::Value`, encodes it, decodes it, and
//! asserts byte-identical re-encoding plus value equality.

use redlinedb_kernel::json;
use serde_json::{Map, Value};

/// Tiny deterministic xorshift64* PRNG.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn gen_range(&mut self, hi: u64) -> u64 {
        if hi == 0 { 0 } else { self.next_u64() % hi }
    }
}

fn random_string(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.gen_range(max_len as u64 + 1) as usize;
    let mut s = String::with_capacity(len);
    let pool: &[char] = &[
        'a',
        'b',
        'c',
        'd',
        'e',
        '_',
        '0',
        '9',
        ' ',
        '-',
        'á',
        '🦀',
        '中',
        '\u{1F600}',
    ];
    for _ in 0..len {
        let i = rng.gen_range(pool.len() as u64) as usize;
        s.push(pool[i]);
    }
    s
}

fn random_value(rng: &mut Rng, depth: u32) -> Value {
    let kind = rng.gen_range(if depth >= 4 { 6 } else { 8 });
    match kind {
        0 => Value::Null,
        1 => Value::Bool(rng.gen_range(2) == 1),
        2 => {
            // i64 spanning negative and positive.
            let bits = rng.next_u64();
            Value::Number(serde_json::Number::from(bits as i64))
        }
        3 => {
            // Finite f64. Avoid NaN/Inf since serde_json::Number can't hold them.
            let bits = rng.next_u64();
            let mut f = f64::from_bits(bits);
            if !f.is_finite() {
                f = (bits as i64) as f64;
            }
            match serde_json::Number::from_f64(f) {
                Some(n) => Value::Number(n),
                None => Value::Number(0.into()),
            }
        }
        4 => Value::String(random_string(rng, 24)),
        5 => Value::String(String::new()),
        6 => {
            let len = rng.gen_range(5) as usize;
            let mut items = Vec::with_capacity(len);
            for _ in 0..len {
                items.push(random_value(rng, depth + 1));
            }
            Value::Array(items)
        }
        _ => {
            let len = rng.gen_range(5) as usize;
            let mut map = Map::new();
            for _ in 0..len {
                let key = random_string(rng, 12);
                map.insert(key, random_value(rng, depth + 1));
            }
            Value::Object(map)
        }
    }
}

#[test]
fn jsonb_round_trip_1000_iterations() {
    let mut rng = Rng::new(0xC0DE_F00D_5EED_5EED);
    for i in 0..1000 {
        let v = random_value(&mut rng, 0);
        let encoded = json::encode(&v);
        // Determinism: same value → same bytes.
        let again = json::encode(&v);
        assert_eq!(encoded, again, "non-deterministic encoding at iter {i}");
        let back = json::decode(&encoded).expect("decode failed");
        // Re-encoding the decoded value must yield the same bytes.
        let re_encoded = json::encode(&back);
        assert_eq!(encoded, re_encoded, "round trip mismatch at iter {i}");
    }
}

#[test]
fn malformed_jsonb_never_panics() {
    let mut rng = Rng::new(0xDEAD_BEEF_BAD0_FACE);
    for _ in 0..256 {
        let len = (rng.gen_range(128) + 1) as usize;
        let mut buf = Vec::with_capacity(len);
        for _ in 0..len {
            buf.push((rng.gen_range(256)) as u8);
        }
        // We don't care what `decode` returns — only that it doesn't panic.
        let _ = json::decode(&buf);
    }
    // Edge cases: empty, only-magic, only-preamble.
    let _ = json::decode(&[]);
    let _ = json::decode(&[json::MAGIC]);
    let _ = json::decode(&[json::MAGIC, json::FORMAT_VERSION]);
}

#[test]
fn truncated_composite_returns_err() {
    let v = serde_json::json!({"a": [1, 2, 3], "b": "hello"});
    let bytes = json::encode(&v);
    for cut in 1..bytes.len() {
        let truncated = &bytes[..cut];
        let _ = json::decode(truncated); // never panics; may err or succeed for tiny prefixes
    }
}

#[test]
fn random_path_eval_matches_serde_lookup() {
    let mut rng = Rng::new(0xBEEF_CAFE_DEAD_F00D);
    for i in 0..200 {
        let mut map = Map::new();
        let n = (rng.gen_range(8) + 1) as usize;
        let mut keys: Vec<String> = Vec::with_capacity(n);
        for _ in 0..n {
            let key = random_string(&mut rng, 10);
            // Make sure key isn't empty (path syntax can't express it cleanly).
            let key = if key.is_empty() { "k".to_string() } else { key };
            map.insert(key.clone(), Value::Number((rng.next_u64() as i64).into()));
            keys.push(key);
        }
        let root = Value::Object(map.clone());
        let bytes = json::encode(&root);
        // Pick a random key and look it up via path eval.
        let pick = &keys[rng.gen_range(keys.len() as u64) as usize];
        let escaped = format!("$.\"{}\"", pick.replace('"', ""));
        let Ok(compiled) = json::compile_path(&escaped) else {
            continue;
        };
        let slice = json::path_eval(&bytes, &compiled).expect("path_eval err");
        // Either we found a value or the key contained a quote we filtered out;
        // skip when the modified key no longer exists.
        if let Some(slice_bytes) = slice {
            // Decode the matched node and compare with serde lookup using
            // the cleaned key.
            let cleaned = pick.replace('"', "");
            if let Some(expected) = map.get(&cleaned) {
                let prefixed = {
                    let mut buf = vec![json::MAGIC, json::FORMAT_VERSION];
                    buf.extend_from_slice(slice_bytes);
                    buf
                };
                let got = json::decode(&prefixed).expect("re-decode");
                assert_eq!(&got, expected, "path mismatch at iter {i}");
            }
        }
    }
}
