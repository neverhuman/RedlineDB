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

# Exact protected PR lane: fast tests plus hard security, dependency, and audit gates.
required:
  ./scripts/ci-local.sh required

setup:
  ./scripts/just/run.sh cache-warm

test:
  ./scripts/just/run.sh fast-test

verify:
  ./scripts/just/run.sh check
