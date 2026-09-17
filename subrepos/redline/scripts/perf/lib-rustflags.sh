# Shared RUSTFLAGS for perf scripts (PGO, BOLT, native).
#
# Sourced — not executed — by scripts/perf/{pgo.sh,bolt.sh,...}.
# Centralises the linker + target-cpu flags so that setting RUSTFLAGS
# in those scripts does not silently *drop* the flags carried by
# .cargo/config.toml (cargo's RUSTFLAGS env var replaces, not appends).
#
# Usage:
#   . "$(git rev-parse --show-toplevel)/scripts/perf/lib-rustflags.sh"
#   RUSTFLAGS="${REDLINE_BASE_RUSTFLAGS} -Cprofile-generate=$PGO_DATA_DIR"

# Linker + target-cpu must stay in lockstep with .cargo/config.toml's
# [target.x86_64-unknown-linux-gnu].rustflags / [target.aarch64-...].
# When that file changes, mirror the change here.
REDLINE_BASE_RUSTFLAGS="-C link-arg=-fuse-ld=mold -C target-cpu=native"
export REDLINE_BASE_RUSTFLAGS
