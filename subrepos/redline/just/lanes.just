fast:
  ./scripts/just/fast.sh

pr-ci:
  ./scripts/ci-local.sh pr-ci

fast-check:
  ./scripts/just/run.sh fast-check

fast-test:
  ./scripts/just/run.sh fast-test

hygiene:
  ./scripts/just/run.sh hygiene

clippy:
  ./scripts/just/run.sh clippy

medium:
  ./scripts/just/run.sh medium

phase8-smoke:
  ./scripts/just/run.sh phase8-smoke

phase9-smoke:
  ./scripts/just/run.sh phase9-smoke

phase9-certify:
  ./scripts/just/run.sh phase9-certify

phase9-xbabe1-gap:
  ./scripts/just/run.sh phase9-xbabe1-gap

phase9-xbabe1-gap-strace:
  ./scripts/just/run.sh phase9-xbabe1-gap-strace

phase11-oltp-gap:
  ./scripts/just/run.sh phase11-oltp-gap

phase11-ephemeral-db:
  ./scripts/just/run.sh phase11-ephemeral-db

phase11-sql-contracts:
  ./scripts/just/run.sh phase11-sql-contracts

phase9-failpoint-matrix:
  ./scripts/just/run.sh phase9-failpoint-matrix

security:
  ./scripts/just/run.sh security

pre-push:
  ./scripts/just/run.sh pre-push

ci-doctor:
  ./scripts/just/run.sh ci-doctor

release-binary-smoke:
  ./scripts/just/run.sh release-binary-smoke

release:
  ./scripts/just/run.sh release

cache-warm:
  ./scripts/just/run.sh cache-warm

fast-nextest:
  ./scripts/just/run.sh fast-nextest

kernel-cursor:
  ./scripts/just/run.sh kernel-cursor

redline-testing-official:
  ./scripts/just/run.sh redline-testing-official

official-evidence-guard:
  ./scripts/just/run.sh official-evidence-guard

sqlite-parity-report-update:
  ./scripts/just/run.sh sqlite-parity-report-update

sqlite-parity-report-check:
  ./scripts/just/run.sh sqlite-parity-report-check

sqlite-parity-report-publish-pr:
  ./scripts/just/run.sh sqlite-parity-report-publish-pr

ffi-abi:
  ./scripts/just/run.sh ffi-abi

cli-shell:
  ./scripts/just/run.sh cli-shell

score:
  ./scripts/just/run.sh score

doctor:
  ./scripts/just/run.sh doctor

rust-map:
  ./scripts/just/run.sh rust-map

rust-witness:
  ./scripts/just/run.sh rust-witness

rust-diagnose:
  ./scripts/just/run.sh rust-diagnose

kernel-check:
  ./scripts/just/run.sh kernel-check

kernel-test:
  ./scripts/just/run.sh kernel-test

sql-check:
  ./scripts/just/run.sh sql-check

sql-test:
  ./scripts/just/run.sh sql-test

beyond-sqlite-manifest:
  ./scripts/just/run.sh beyond-sqlite-manifest

beyond-postgres-reference:
  ./scripts/just/run.sh beyond-postgres-reference

ffi-check:
  ./scripts/just/run.sh ffi-check

ffi-test:
  ./scripts/just/run.sh ffi-test

cli-check:
  ./scripts/just/run.sh cli-check

cli-test:
  ./scripts/just/run.sh cli-test

# Full-corpus performance measurement through the verified external runner.
# 1127-case corpus × 3 reps × auto workers (~3h; mirrors CI).
perf-full BIN="target/release/redlinedb" OUT="candidate":
  ./scripts/perf/full.sh {{BIN}} {{OUT}}

# W2 build/profile matrix. The default builds only; SUITE=full opts into
# complete external-corpus measurements.
perf-w2-matrix SUITE="none" PROFILES="release,release-native" ALLOCATORS="mimalloc,jemalloc":
  bash scripts/perf/w2-matrix.sh --suite {{SUITE}} --profiles {{PROFILES}} --allocators {{ALLOCATORS}}

# Phase 5 WS-B perf lanes — PGO + BOLT post-link.
# perf-pgo: trains on the complete external corpus and produces
#           target/release-pgo/redlinedb. Pass --for-bolt directly to
#           scripts/perf/pgo.sh to emit relocations needed by perf-bolt.
# perf-bolt: requires perf-pgo to have run first WITH --for-bolt so
#            the input binary has --emit-relocs sections; samples the
#            PGO'd binary under the parity workload via `perf record`
#            and rewrites it with llvm-bolt. x86_64 only.
perf-pgo:
  bash scripts/perf/pgo.sh

perf-bolt:
  bash scripts/perf/bolt.sh
