#!/usr/bin/env bash
set -euo pipefail

PROFILE="${1:-bounded}"
REMOTE="${REMOTE:-xbabe1}"
REMOTE_DIR="${REMOTE_DIR:-/home/ubuntu/RedlineDB}"

case "${PROFILE}" in
  bounded)
    CONFIG="crates/bench/bench/dick-head-choas-bounded.toml"
    ;;
  extreme)
    CONFIG="crates/bench/bench/dick-head-choas-extreme.toml"
    ;;
  *)
    echo "unknown profile ${PROFILE}" >&2
    exit 1
    ;;
esac

join_csv() {
  local joined=""
  local sep=""
  local value
  for value in "$@"; do
    joined+="${sep}${value}"
    sep=", "
  done
  printf '%s' "${joined}"
}

join_quoted_csv() {
  local joined=""
  local sep=""
  local value
  for value in "$@"; do
    joined+="${sep}\"${value}\""
    sep=", "
  done
  printf '%s' "${joined}"
}

build_thread_list() {
  local max_threads="$1"
  local -a candidates=()
  local value
  if [ "${PROFILE}" = "extreme" ]; then
    candidates=(64 128 200 256 384 512)
  else
    candidates=(16 64 128 200 256)
  fi
  for value in "${candidates[@]}"; do
    if [ "${value}" -le "${max_threads}" ]; then
      THREADS+=("${value}")
    fi
  done
}

STAMP="dick-head-choas-${PROFILE}-$(date +%Y%m%d-%H%M%S)"

./scripts/bench/xbabe1_sync.sh

./scripts/bench/xbabe1_run.sh cargo run -p redlinedb-bench --release -- certify \
  --config "${CONFIG}" \
  --out-dir "target/bench/xbabe1/${STAMP}" \
  --seed 7 \
  --repetitions $([ "${PROFILE}" = "extreme" ] && printf '1' || printf '2') \
  --warmup $([ "${PROFILE}" = "extreme" ] && printf '0' || printf '1')

./scripts/bench/xbabe1_fetch.sh "${STAMP}"
python3 scripts/bench/export_benchmark_results.py

ARCHIVE="/tmp/redlinedb-xbabe1-${STAMP}.tar.gz"
tar -czf "${ARCHIVE}" -C target/bench/xbabe1 "${STAMP}"
shasum -a 256 "${ARCHIVE}"
