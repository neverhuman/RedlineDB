set shell := ["bash", "-euo", "pipefail", "-c"]
export RUSTC_WRAPPER := "./scripts/sccache_wrapper.sh"

import 'just/lanes.just'

default: fast

check:
  ./scripts/just/run.sh fast
  ./scripts/just/run.sh score
  ./scripts/just/run.sh security
  ./scripts/just/run.sh rust-map
  ./scripts/just/run.sh rust-witness
  ./scripts/just/run.sh rust-diagnose

setup:
  ./scripts/just/run.sh cache-warm

test:
  ./scripts/just/run.sh fast-test

fast-score:
  mkdir -p target/jankurai
  if rtk cargo metadata --no-deps --format-version 1 --locked | rtk rg -q '"name":"jankurai"'; then rtk cargo check -p jankurai --locked; else rtk cargo check -p redlinedb-kernel --locked; fi
  jankurai . --json target/jankurai/fast-score.json --md target/jankurai/fast-score.md --no-score-history

audit-fast:
  mkdir -p target/jankurai
  jankurai audit . --mode advisory --changed-fast --changed-from HEAD --json target/jankurai/audit-fast.json --md target/jankurai/audit-fast.md --no-score-history

verify:
  ./scripts/just/run.sh check
