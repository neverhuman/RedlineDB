#!/usr/bin/env bash
# Authoritative local CI dispatcher used by Jeryu host CI.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v rtk >/dev/null 2>&1; then
    rtk() {
        "$@"
    }
fi

usage() {
    cat >&2 <<'USAGE'
usage: scripts/ci-local.sh {required|pr-ci|security|jankurai|audit|release|doctor}

  required alias for the pr-ci validation surface
  pr-ci    run the exact local mirror of the CI validation surface
  security run the repository security lane
  jankurai run the jankurai tool-suite evidence lane
  audit    run the repository audit lane
  release  run the release packaging lane
  doctor   run the workflow and hook sanity checks
USAGE
}

if [ "$#" -ne 1 ]; then
    usage
    exit 64
fi

case "$1" in
    required|pr-ci)
        bash "$repo_root/ops/ci/pr-ci.sh"
        ;;
    security)
        bash "$repo_root/ops/ci/security.sh"
        ;;
    jankurai)
        bash "$repo_root/ops/ci/jankurai.sh"
        ;;
    audit)
        bash "$repo_root/ops/ci/jankurai-audit.sh"
        ;;
    release)
        bash "$repo_root/ops/ci/release.sh"
        ;;
    doctor)
        bash "$repo_root/scripts/ci-doctor.sh"
        ;;
    -h|--help|help)
        usage
        ;;
    *)
        printf 'ci-local: unknown lane %q\n\n' "$1" >&2
        usage
        exit 64
        ;;
esac
