# Performance build profiles

Three cargo profiles are available for the redlinedb binary, in order of increasing perf and decreasing portability.

| Profile | When | How |
|---|---|---|
| `release` | Default, what we ship | `cargo build --release` |
| `release-native` | Local benches on the build host | `RUSTFLAGS="-C target-cpu=native" cargo build --profile release-native -p redlinedb-cli` |
| `release-pgo` | Reproducible bench numbers for PR descriptions | `scripts/perf/pgo.sh` (two-pass: instrument → train → recompile) |

All three inherit `lto = "thin"`, `codegen-units = 1`, and `panic = "abort"` from `release`.

## `release-native`

Inherits `release`. The `target-cpu=native` rustflag must be supplied via env (Cargo does not yet support per-profile rustflags on stable). Use only when the binary will run on the same CPU it was built on — a binary built on Skylake will not run on Haswell.

## `release-pgo`

Inherits `release-native`. Profile-guided optimization trains on the parity workload via the `redline-testing` runner. Two passes — instrumented build, training run, final build — managed by `scripts/perf/pgo.sh`.

## Per-binary builds

For the CLI binary specifically:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo build --profile release-native -p redlinedb-cli --bin redlinedb
```

Output lands at `target/release-native/redlinedb` (the directory name matches the profile, not `release/`).
