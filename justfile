set shell := ["bash", "-euo", "pipefail", "-c"]
export RUSTC_WRAPPER := "./scripts/sccache_wrapper.sh"

import 'just/lanes.just'

default: fast

check: cache-warm-root fast-check-root fast-test-root
  ./scripts/just/run.sh fast
  ./scripts/just/run.sh score
  ./scripts/just/run.sh security
  ./scripts/just/run.sh rust-map
  ./scripts/just/run.sh rust-witness
  ./scripts/just/run.sh rust-diagnose

setup: cache-warm-root
  ./scripts/just/run.sh cache-warm

cache-warm-root:
  ./scripts/just/run.sh cache-warm

fast-check-root:
  ./scripts/just/run.sh fast-check

fast-test-root:
  ./scripts/just/run.sh fast-test

test: fast-test-root
  ./scripts/just/run.sh fast-test

verify:
  ./scripts/just/run.sh check
