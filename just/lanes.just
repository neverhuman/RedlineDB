fast:
  ./scripts/just/run.sh fast

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

sql-parity:
  ./scripts/just/run.sh sql-parity

sql-parity-full:
  ./scripts/just/run.sh sql-parity-full

sqlite-parity-scale-smoke:
  ./scripts/just/run.sh sqlite-parity-scale-smoke

sqlite-parity-scale-ci:
  ./scripts/just/run.sh sqlite-parity-scale-ci

sqlite-parity-report-update:
  ./scripts/just/run.sh sqlite-parity-report-update

sqlite-parity-report-check:
  ./scripts/just/run.sh sqlite-parity-report-check

sqlite-parity-scale-full:
  ./scripts/just/run.sh sqlite-parity-scale-full

ffi-abi:
  ./scripts/just/run.sh ffi-abi

ffi-parity-full:
  ./scripts/just/run.sh ffi-parity-full

ffi-symbol-diff:
  ./scripts/just/run.sh ffi-symbol-diff

cli-shell:
  ./scripts/just/run.sh cli-shell

cli-parity-full:
  ./scripts/just/run.sh cli-parity-full

fuzz-parity:
  ./scripts/just/run.sh fuzz-parity

fuzz-parity-nightly:
  ./scripts/just/run.sh fuzz-parity-nightly

parity-full:
  ./scripts/just/run.sh parity-full

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

ffi-check:
  ./scripts/just/run.sh ffi-check

ffi-test:
  ./scripts/just/run.sh ffi-test

cli-check:
  ./scripts/just/run.sh cli-check

cli-test:
  ./scripts/just/run.sh cli-test
