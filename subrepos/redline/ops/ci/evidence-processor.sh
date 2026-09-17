#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

manifest="tools/evidence-processor/Cargo.toml"
target_dir="$repo_root/target/evidence-processor"

cargo fmt --check --manifest-path "$manifest"
cargo test --locked --manifest-path "$manifest" --target-dir "$target_dir"

for wrapper in \
  tools/evidence-processor/run.sh \
  tools/jankurai-hooks/check-score-ratchet.sh \
  ops/ci/artifact_support.sh \
  ops/ci/jankurai-audit.sh \
  ops/ci/jankurai-tools.sh \
  ops/deploy/telemetry.sh \
  scripts/ci-doctor.sh \
  scripts/perf/lib.sh \
  scripts/perf/w2-matrix.sh \
  scripts/process-redline-testing-evidence.sh
do
  bash -n "$wrapper"
done

empty_root="$(mktemp -d)"
cleanup() {
  rm -rf "$empty_root"
}
trap cleanup EXIT

if cargo run --locked --quiet \
  --manifest-path "$manifest" \
  --target-dir "$target_dir" \
  -- process-official "$empty_root" >"$target_dir/missing-input.stdout" 2>"$target_dir/missing-input.stderr"
then
  printf 'bundled evidence processor accepted a missing evidence bundle\n' >&2
  exit 1
fi
grep -q 'official-evidence.json' "$target_dir/missing-input.stderr"

if cargo run --locked --quiet \
  --manifest-path "$manifest" \
  --target-dir "$target_dir" \
  -- telemetry --unknown >"$target_dir/telemetry-usage.stdout" 2>"$target_dir/telemetry-usage.stderr"
then
  printf 'telemetry control accepted an unknown argument\n' >&2
  exit 1
else
  rc=$?
fi
if [ "$rc" -ne 2 ]; then
  printf 'telemetry usage error returned %s instead of 2\n' "$rc" >&2
  exit 1
fi
grep -q 'telemetry: unknown arg' "$target_dir/telemetry-usage.stderr"
