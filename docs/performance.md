# Performance Build Profiles

Three cargo profiles are available for the `redlinedb` binary, in order
of increasing perf and decreasing portability.

| Profile | When | How |
|---|---|---|
| `release` | Default, what we ship | `cargo build --release` |
| `release-native` | Local benches on the build host | `RUSTFLAGS="-C target-cpu=native" cargo build --profile release-native -p redlinedb-cli` |
| `release-pgo` | Reproducible bench numbers for PR descriptions | `scripts/perf/pgo.sh` (two-pass: instrument → train → recompile) |

All three inherit `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`,
`panic = "abort"`, and stripped symbols from `release`.

The checked-in Linux x86_64 default in `.cargo/config.toml` is
`target-cpu=x86-64-v3` for portability. The perf scripts intentionally
override that with `target-cpu=native` through
`scripts/perf/lib-rustflags.sh`; binaries produced by those scripts are
benchmark artifacts for the build host, not portable release artifacts.

## `release-native`

Inherits `release`. The `target-cpu=native` rustflag must be supplied via
env because Cargo does not support per-profile rustflags on stable. Use
only when the binary will run on the same CPU it was built on.

## `release-pgo`

Inherits `release-native`. Profile-guided optimization trains on the
parity workload via the `redline-testing` runner. Two passes -
instrumented build, training run, final build - are managed by
`scripts/perf/pgo.sh`.

`scripts/perf/pgo.sh` accepts `REDLINE_CARGO_FEATURE_ARGS` for local
allocator A/B runs:

```bash
REDLINE_CARGO_FEATURE_ARGS="--no-default-features --features alloc-jemalloc" \
  scripts/perf/pgo.sh --training-subset quick
```

By default the script also allocates unique temporary PGO data/profile
directories per run, which avoids collisions when multiple perf jobs are
active. Override `PGO_DATA_DIR` and `PGO_PROFILE_DIR` if you want to
pin the output locations.

## W2 Matrix

Use `scripts/perf/w2-matrix.sh` for repeatable W2 build/profile runs. It
builds selected profile/allocator variants, copies each binary under
`target/perf/w2-matrix/<run-id>/bin/`, optionally runs a perf lane, and
writes one manifest row per variant to `manifest.jsonl`.

The default matrix is bounded:

```bash
just perf-w2-matrix
```

Heavier runs can opt into PGO and BOLT explicitly:

```bash
scripts/perf/w2-matrix.sh \
  --suite medium \
  --profiles release,release-native,release-pgo,release-pgo-bolt \
  --allocators mimalloc,jemalloc \
  --pgo-training-subset medium
```

The matrix passes the allocator feature set through to `pgo.sh`, so the
`release-pgo` and `release-pgo-bolt` legs train and rebuild under the
selected allocator as well.

Allocator choices are currently the mutually exclusive CLI features
`alloc-mimalloc`, `alloc-jemalloc`, and `alloc-snmalloc`. There is no
system-allocator feature yet, so the W2 matrix does not claim a system
allocator leg.

## Per-binary builds

For the CLI binary specifically:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo build --profile release-native -p redlinedb-cli --bin redlinedb
```

Output lands at `target/release-native/redlinedb` (the directory name matches the profile, not `release/`).
