// WS-B4: A/B allocator selection via cargo features. Normal builds enable
// exactly one of alloc-mimalloc / alloc-jemalloc / alloc-snmalloc. Cargo's
// release qualification uses --all-features; that exact all-three combination
// deterministically selects the default mimalloc while compiling every
// optional allocator dependency.
#[cfg(not(any(
    feature = "alloc-mimalloc",
    feature = "alloc-jemalloc",
    feature = "alloc-snmalloc"
)))]
compile_error!(
    "redlinedb-cli requires exactly one allocator feature: alloc-mimalloc, alloc-jemalloc, or alloc-snmalloc"
);

#[cfg(any(
    all(
        feature = "alloc-mimalloc",
        feature = "alloc-jemalloc",
        not(feature = "alloc-snmalloc")
    ),
    all(
        feature = "alloc-mimalloc",
        feature = "alloc-snmalloc",
        not(feature = "alloc-jemalloc")
    ),
    all(
        feature = "alloc-jemalloc",
        feature = "alloc-snmalloc",
        not(feature = "alloc-mimalloc")
    ),
))]
compile_error!(
    "redlinedb-cli allocator features are mutually exclusive: enable only one of alloc-mimalloc / alloc-jemalloc / alloc-snmalloc"
);

#[cfg(feature = "alloc-mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(not(feature = "alloc-mimalloc"), feature = "alloc-jemalloc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(all(
    not(feature = "alloc-mimalloc"),
    not(feature = "alloc-jemalloc"),
    feature = "alloc-snmalloc"
))]
#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

fn main() {
    redlinedb_cli::run();
}
