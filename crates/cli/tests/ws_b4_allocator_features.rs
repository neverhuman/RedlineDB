// WS-B4: smoke test that the allocator-feature-gated build links and runs.
// The actual A/B selection is exercised by `cargo check --no-default-features
// --features alloc-{mimalloc,jemalloc,snmalloc}` in CI / by hand. This test
// just confirms the default-features build produces a working binary that
// does a trivial allocation under whichever allocator is active.

#[test]
fn allocator_feature_smoke_alloc_and_drop() {
    // Force a heap allocation through the configured global allocator.
    let v: Vec<u64> = (0..1024).collect();
    assert_eq!(v.len(), 1024);
    assert_eq!(v[1023], 1023);

    // Boxed allocation as well, to stress a different size class.
    let boxed: Box<[u8; 4096]> = Box::new([0u8; 4096]);
    assert_eq!(boxed.len(), 4096);
}

#[test]
fn allocator_feature_string_churn() {
    // Repeated alloc/free cycles to make sure the active allocator handles
    // mixed sizes without panicking.
    let mut acc = String::new();
    for i in 0..256 {
        let s = format!("redlinedb-ws-b4-{i}");
        acc.push_str(&s);
    }
    assert!(acc.len() > 256);
}
