#!/usr/bin/env bash
set -euo pipefail

# Lane BH P1 #4: capture the host-side git state BEFORE we ssh, then
# pass it through to the docker container via env. The container does
# not have a `.git` directory (xbabe1_sync.sh excludes it), so the
# bench process inside cannot derive these on its own. The bench
# `report.rs::collect_environment` reads these env vars first and
# only falls back to `git` shellouts when the env is absent — so the
# manifest gets the host SHA on remote runs and stays correct on
# local runs that still have `.git` available.

if [ "$#" -eq 0 ]; then
  echo "usage: xbabe1_run.sh <docker command...>" >&2
  exit 1
fi

REMOTE="${REMOTE:-xbabe1}"
REMOTE_DIR="${REMOTE_DIR:-/home/ubuntu/RedlineDB}"
IMAGE="${IMAGE:-redlinedb-bench:1.95.0}"
REMOTE_COMMAND="$(printf '%q ' "$@")"

# Capture the host-side commit and dirty bit. These run BEFORE any
# remote work so a failure here surfaces locally with a clear shell
# error instead of an opaque "missing git_sha" in the manifest.
REDLINEDB_BENCH_GIT_SHA="$(git rev-parse HEAD)"
REDLINEDB_BENCH_GIT_SHORT="$(git rev-parse --short HEAD)"
if git diff-index --quiet HEAD --; then
  REDLINEDB_BENCH_GIT_DIRTY=false
else
  REDLINEDB_BENCH_GIT_DIRTY=true
fi
export REDLINEDB_BENCH_GIT_SHA REDLINEDB_BENCH_GIT_SHORT REDLINEDB_BENCH_GIT_DIRTY

# Build the image, then capture its digest so the certify manifest can record
# the exact image used. Prefer a RepoDigest (set when the image has been
# pushed/pulled); fall back to the local image ID otherwise.
ssh "${REMOTE}" "cd '${REMOTE_DIR}' && docker build -f crates/bench/docker/Dockerfile -t '${IMAGE}' ."

REDLINEDB_BENCH_IMAGE_DIGEST="$(ssh "${REMOTE}" "docker inspect --format '{{if .RepoDigests}}{{index .RepoDigests 0}}{{else}}{{.Id}}{{end}}' '${IMAGE}'")"
export REDLINEDB_BENCH_IMAGE_DIGEST

ssh "${REMOTE}" "cd '${REMOTE_DIR}' && docker run --rm -u \$(id -u):\$(id -g) \
  -e CARGO_TARGET_DIR=/work/target \
  -e PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  -e REDLINEDB_BENCH_IMAGE_DIGEST='${REDLINEDB_BENCH_IMAGE_DIGEST}' \
  -e REDLINEDB_BENCH_GIT_SHA='${REDLINEDB_BENCH_GIT_SHA}' \
  -e REDLINEDB_BENCH_GIT_SHORT='${REDLINEDB_BENCH_GIT_SHORT}' \
  -e REDLINEDB_BENCH_GIT_DIRTY='${REDLINEDB_BENCH_GIT_DIRTY}' \
  -v /tmp:/tmp \
  -v '${REMOTE_DIR}':/work -w /work '${IMAGE}' \
  bash -c $(printf '%q' "${REMOTE_COMMAND}")"
